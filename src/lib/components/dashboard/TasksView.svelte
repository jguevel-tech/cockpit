<script lang="ts">
  import { onMount } from "svelte";
  import { createTodo, getPendingTodos, reorderTodos, moveTodo, updateTodo, deleteTodo, setTodoDue, setTodoProgress, renommerListeDeTaches, supprimerListeDeTaches } from "../../api/storage";
  import { getAppSettings, setAppSetting } from "../../api/recorder";
  import { SANS_PROJET, CLE_REGLAGE_LISTES, estUneListe, cleDeListe, nomDeListe, lireListes, ordonnerLesListes } from "../../todos/listes";
  import { demanderTexte } from "../../stores/saisie";
  import { reorderProjects } from "../../api/scanner";
  import { selectProject, activeTab } from "../../stores/ui";
  import { projects, loadProjects } from "../../stores/projects";
  import { reorderable } from "../../actions/reorderable";
  import { reorder, groupBy, type DropPosition } from "../../utils/reorder";
  import { notify } from "../../stores/toast";
  import { dueLabel, dueUrgency } from "../../utils/due";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import TodoText from "../todos/TodoText.svelte";
  import TodoProgress from "../todos/TodoProgress.svelte";
  import TodoSaisie from "../todos/TodoSaisie.svelte";
  import type { Todo } from "../../types";
  import { trad, tradN } from "../../i18n";
  import { demanderConfirmation } from "../../stores/confirm";

  let pendingTodos: Todo[] = $state([]);

  /// Cles des listes hors projet, dans l'ordre choisi (reglage local).
  let listesRetenues: string[] = $state([]);

  /// Les listes d'abord, puis les projets. Une liste retenue s'affiche meme vide : on vient de
  /// la creer, ou de cocher sa derniere tache, et elle ne doit pas disparaitre pour autant.
  let groupedTodos: { project: string; todos: Todo[] }[] = $derived.by(() => {
    const dansDesListes = pendingTodos.filter((t) => estUneListe(t.project));
    const vues = [...new Set(dansDesListes.map((t) => t.project))];
    const listes = ordonnerLesListes(listesRetenues, vues)
      .map((cle) => ({ project: cle, todos: dansDesListes.filter((t) => t.project === cle) }));
    const projets = groupBy(pendingTodos.filter((t) => !estUneListe(t.project)), (t) => t.project)
      .map((g) => ({ project: g.key, todos: g.items }));
    return [...listes, ...projets];
  });

  /// Ou va une tache tapee en haut : une liste nommee, ou « Hors projet ».
  let cibles: string[] = $derived([
    ...groupedTodos.map((g) => g.project).filter((p) => p !== SANS_PROJET && estUneListe(p)),
    SANS_PROJET,
  ]);
  let cible = $state<string | null>(null);
  let cibleEffective = $derived(cible !== null && cibles.includes(cible) ? cible : cibles[0]);

  function libelleDeListe(cle: string): string {
    return nomDeListe(cle) ?? $trad("tasks.sansProjet");
  }

  let totalTodoCount: number = $derived(pendingTodos.length);

  onMount(async () => {
    try {
      const [todos, reglages] = await Promise.all([getPendingTodos(), getAppSettings()]);
      pendingTodos = todos;
      listesRetenues = lireListes(reglages[CLE_REGLAGE_LISTES]);
    } catch (e) { notify(String(e)); }
  });

  async function enregistrerLesListes(listes: string[]) {
    listesRetenues = listes;
    try { await setAppSetting(CLE_REGLAGE_LISTES, JSON.stringify(listes)); }
    catch (e) { notify(String(e), "error", 4000, { scope: "taches.listes" }); }
  }

  function listeExiste(cle: string): boolean {
    return cle === SANS_PROJET ? false : groupedTodos.some((g) => g.project === cle) || listesRetenues.includes(cle);
  }

  async function nouvelleListe() {
    const nom = await demanderTexte({ message: $trad("tasks.listNamePrompt"), action: $trad("common.create") });
    if (nom === null) return;
    const cle = cleDeListe(nom);
    if (listeExiste(cle)) { notify($trad("tasks.listExists"), "info", 4000, { report: false }); return; }
    await enregistrerLesListes([...groupedTodos.map((g) => g.project).filter((p) => p !== SANS_PROJET && estUneListe(p)), cle]);
    cible = cle;
  }

  /// Renommer « Hors projet » en fait une liste nommee : ses taches la suivent.
  async function renommerListe(cle: string) {
    const nom = await demanderTexte({
      message: $trad("tasks.listRenamePrompt"),
      action: $trad("common.rename"),
      valeur: nomDeListe(cle) ?? "",
    });
    if (nom === null) return;
    const nouvelle = cleDeListe(nom);
    if (nouvelle === cle) return;
    if (listeExiste(nouvelle)) { notify($trad("tasks.listExists"), "info", 4000, { report: false }); return; }
    try {
      await renommerListeDeTaches(cle, nouvelle);
    } catch (e) { notify(String(e)); return; }
    const ordre = groupedTodos.map((g) => g.project).filter((p) => estUneListe(p));
    await enregistrerLesListes(ordre.map((p) => (p === cle ? nouvelle : p)).filter((p) => p !== SANS_PROJET));
    if (cibleEffective === cle) cible = nouvelle;
    await reload();
  }

  async function supprimerListe(cle: string, enCours: number) {
    const question = $trad("tasks.listDeleteConfirm", { nom: libelleDeListe(cle), n: enCours });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    try { await supprimerListeDeTaches(cle); } catch (e) { notify(String(e)); return; }
    await enregistrerLesListes(listesRetenues.filter((p) => p !== cle));
    await reload();
  }

  async function reload() {
    try { pendingTodos = await getPendingTodos(); } catch (e) { notify(String(e)); }
  }

  /// Une idee notee au vol, dans la liste choisie a cote du champ.
  async function ajouterHorsProjet(texte: string): Promise<boolean> {
    try { await createTodo(cibleEffective, texte); await reload(); return true; }
    catch (e) { notify(String(e)); return false; }
  }

  async function toggleDone(todo: Todo) {
    // Optimistic UI: retire le todo de la liste immediatement
    pendingTodos = pendingTodos.filter((t) => t.id !== todo.id);
    try {
      await updateTodo(todo.id, todo.text, true);
    } catch (e) {
      // Rollback en cas d'erreur
      notify(String(e));
      await reload();
    }
  }

  // Meme raison que dans la colonne Todos : la ligne en cours de reglage n'est plus glissable.
  let reglageEnCours: number | null = $state(null);

  async function removeTodo(todo: Todo) {
    const question = $trad("todos.deleteConfirm", { texte: todo.text });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    pendingTodos = pendingTodos.filter((t) => t.id !== todo.id);
    try { await deleteTodo(todo.id); } catch (e) { notify(String(e)); await reload(); }
  }

  let editingTodoId: number | null = $state(null);

  function startEditTodo(todo: Todo) {
    editingTodoId = todo.id;
  }

  async function commitEditTodo(todo: Todo, next: string) {
    editingTodoId = null;
    const newValue = next.trim();
    if (!newValue || newValue === todo.text) return;
    // Optimistic update
    pendingTodos = pendingTodos.map((t) => (t.id === todo.id ? { ...t, text: newValue } : t));
    try { await updateTodo(todo.id, newValue, todo.done); } catch (e) { notify(String(e)); await reload(); }
  }

  function cancelEditTodo() {
    editingTodoId = null;
  }

  // --- Echeances ---
  // Le badge etait un <span> inerte ici alors qu'il ouvre le calendrier dans la colonne Todos :
  // on voyait l'echeance depuis le tableau de bord sans pouvoir la deplacer, ni en poser une.
  // C'est pourtant l'ecran ou on trie ses taches.
  let editingDueId: number | null = $state(null);

  async function commitDue(todo: Todo, value: string) {
    editingDueId = null;
    const due = value || null;
    if (due === todo.due_date) return;
    // Optimiste : la tache reste en place, on ne renvoie pas l'utilisateur en haut de liste.
    pendingTodos = pendingTodos.map((t) => (t.id === todo.id ? { ...t, due_date: due } : t));
    try { await setTodoDue(todo.id, due); } catch (e) { notify(String(e)); await reload(); }
  }

  /// Avancement d'une tache. En dessous de 100 % on met a jour SUR PLACE, comme pour
  /// l'echeance : recharger ferait sauter la liste sous le curseur. A 100 % la tache devient
  /// finie, donc elle doit quitter la liste des en-cours — la, il faut recharger.
  async function commitProgress(todo: Todo, valeur: number) {
    if (valeur === todo.progress) return;
    if (valeur < 100) {
      pendingTodos = pendingTodos.map((t) => (t.id === todo.id ? { ...t, progress: valeur } : t));
    }
    try {
      await setTodoProgress(todo.id, valeur);
      if (valeur >= 100) await reload();
    } catch (e) {
      notify(String(e));
      await reload();
    }
  }

  // --- Drag & Drop des groupes (projets) via l'action partagee ---
  function onGroupReorder(from: number, to: number, pos: DropPosition) {
    const draggedProject = groupedTodos[from]?.project;
    const targetProject = groupedTodos[to]?.project;
    if (draggedProject === undefined || targetProject === undefined) return;
    // Les listes se rangent entre elles, dans le reglage ; les projets entre eux, en base.
    // Melanger les deux n'a pas de sens : les listes restent toujours en tete.
    if (estUneListe(draggedProject) !== estUneListe(targetProject)) return;
    if (estUneListe(draggedProject)) {
      const ordre = reorder(groupedTodos.map((g) => g.project), from, to, pos)
        .filter((p) => p !== SANS_PROJET && estUneListe(p));
      void enregistrerLesListes(ordre);
      return;
    }

    // On reordonne le store des projets sous-jacent (superset des groupes de todos)
    const currentNames = $projects.map((p) => p.name);
    const srcIdx = currentNames.indexOf(draggedProject);
    if (srcIdx === -1) return;

    const items = [...currentNames];
    items.splice(srcIdx, 1);
    let targetIdx = items.indexOf(targetProject);
    if (targetIdx === -1) { targetIdx = items.length; }
    else if (pos === "after") targetIdx++;
    items.splice(targetIdx, 0, draggedProject);

    reorderProjects(items).then(() => loadProjects()).catch(() => loadProjects());
    // Optimistiquement, recharge les todos (l'ordre suit celui des projets)
    reload();
  }

  // --- Drag & Drop des todos (manuel : gere le deplacement inter-projet) ---
  let dragTodo: { todoId: number; fromProject: string } | null = $state(null);
  let todoDropTarget: { todoId: number; pos: DropPosition } | null = $state(null);
  let groupMoveTarget: string | null = $state(null);

  function onTodoDragStart(e: DragEvent, todo: Todo) {
    dragTodo = { todoId: todo.id, fromProject: todo.project };
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", String(todo.id));
  }

  function onTodoDragOver(e: DragEvent, todo: Todo) {
    if (!dragTodo) return;
    e.preventDefault();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos: DropPosition = e.clientY < rect.top + rect.height / 2 ? "before" : "after";
    todoDropTarget = { todoId: todo.id, pos };
  }

  function onTodoDragLeave() {
    todoDropTarget = null;
  }

  function onTodoDrop(e: DragEvent, targetProject: string) {
    e.preventDefault();
    if (!dragTodo || !todoDropTarget) { cleanTodoDrag(); return; }

    const srcProject = dragTodo.fromProject;
    const todoId = dragTodo.todoId;

    if (srcProject !== targetProject) {
      // Deplacement vers un autre projet puis reload
      moveTodo(todoId, targetProject).then(() => reload()).catch(() => reload());
      cleanTodoDrag();
      return;
    }

    // Meme projet : reorder
    const group = groupedTodos.find((g) => g.project === targetProject);
    if (!group) { cleanTodoDrag(); return; }

    const srcIdx = group.todos.findIndex((t) => t.id === todoId);
    const tgtIdx = group.todos.findIndex((t) => t.id === todoDropTarget!.todoId);
    if (srcIdx === -1 || tgtIdx === -1) { cleanTodoDrag(); return; }

    const items = reorder(group.todos, srcIdx, tgtIdx, todoDropTarget.pos);

    // Optimistic update: remplace en place pour preserver l'ordre des projets
    let replaced = false;
    const updated: Todo[] = [];
    for (const t of pendingTodos) {
      if (t.project === targetProject) {
        if (!replaced) { updated.push(...items); replaced = true; }
      } else {
        updated.push(t);
      }
    }
    pendingTodos = updated;

    reorderTodos(items.map((t) => t.id)).catch(() => reload());
    cleanTodoDrag();
  }

  // Depot d'un todo sur l'en-tete d'un groupe = deplacer vers ce projet
  function onGroupHeaderDragOver(e: DragEvent, project: string) {
    if (!dragTodo) return;
    e.preventDefault();
    groupMoveTarget = project;
  }

  function onGroupHeaderDragLeave() {
    groupMoveTarget = null;
  }

  function onGroupHeaderDrop(e: DragEvent, project: string) {
    if (!dragTodo) return; // laisse remonter pour le drag de groupe
    e.preventDefault();
    e.stopPropagation();
    if (dragTodo.fromProject === project) { cleanTodoDrag(); return; }
    moveTodo(dragTodo.todoId, project).then(() => reload()).catch(() => reload());
    cleanTodoDrag();
  }

  function cleanTodoDrag() {
    dragTodo = null;
    todoDropTarget = null;
    groupMoveTarget = null;
  }
</script>

<div class="todos-panel">
  <div class="panel-header">
    <h3>{$trad("tasks.title")}</h3>
    <span class="task-count">{$tradN("tasks.count", totalTodoCount)}</span>
  </div>
  <div class="saisie">
    <div class="saisie-champ">
      <TodoSaisie placeholder={$trad("tasks.newPlaceholder")} onAdd={ajouterHorsProjet} />
    </div>
    <select
      class="saisie-cible"
      value={cibleEffective}
      title={$trad("tasks.listTarget")}
      aria-label={$trad("tasks.listTarget")}
      onchange={(e) => (cible = (e.currentTarget as HTMLSelectElement).value)}
    >
      {#each cibles as c (c)}<option value={c}>{libelleDeListe(c)}</option>{/each}
    </select>
    <button class="nouvelle-liste" onclick={nouvelleListe} title={$trad("tasks.newListHint")}>{$trad("tasks.newList")}</button>
  </div>
  <div class="todos-list">
    {#each groupedTodos as group, gi}
      <!-- Le groupe hors projet ne se range pas parmi les projets : il n'en est pas un, et il
           reste en tete, la ou on vient de l'ecrire. -->
      <div
        class="todo-group"
        role="listitem"
        use:reorderable={{ index: gi, group: "task-groups", onDrop: onGroupReorder, disabled: group.project === SANS_PROJET }}
      >
        <div
          class="group-header-wrap"
          class:drag-target-group={groupMoveTarget === group.project}
          role="group"
          ondragover={(e) => onGroupHeaderDragOver(e, group.project)}
          ondragleave={onGroupHeaderDragLeave}
          ondrop={(e) => onGroupHeaderDrop(e, group.project)}
        >
          <!-- Depart depuis une tache : on va la voir, donc sur l'onglet qui porte les taches.
               Cette intention passe devant l'onglet memorise du projet. -->
          {#if estUneListe(group.project)}
            <!-- Aucun projet a ouvrir : un en-tete qui ne mene nulle part ne doit pas se
                 presenter comme un bouton. Ses vrais boutons renomment et suppriment. -->
            <div class="group-header liste" title={$trad("tasks.sansProjetHint")}>
              <span class="group-name">{libelleDeListe(group.project)}</span>
              <span class="liste-actions">
                <button class="liste-btn" onclick={() => renommerListe(group.project)}
                  title={$trad("tasks.listRename")} aria-label={$trad("tasks.listRename")}>✎</button>
                <button class="liste-btn danger" onclick={() => supprimerListe(group.project, group.todos.length)}
                  title={$trad("tasks.listDelete")} aria-label={$trad("tasks.listDelete")}>×</button>
              </span>
              <span class="group-count">{group.todos.length}</span>
            </div>
          {:else}
            <button class="group-header" onclick={() => { selectProject(group.project); activeTab.set("workspace"); }}>
              <span class="group-name">{group.project}</span>
              <span class="group-count">{group.todos.length}</span>
            </button>
          {/if}
        </div>
        {#if group.todos.length === 0}
          <p class="liste-vide">{$trad("tasks.listEmpty")}</p>
        {/if}
        {#each group.todos as todo}
          <div
            class="todo-item"
            class:dragging={dragTodo?.todoId === todo.id}
            class:drag-over-top={todoDropTarget?.todoId === todo.id && todoDropTarget?.pos === "before"}
            class:drag-over-bottom={todoDropTarget?.todoId === todo.id && todoDropTarget?.pos === "after"}
            role="listitem"
            draggable={reglageEnCours !== todo.id}
            ondragstart={(e) => { e.stopPropagation(); onTodoDragStart(e, todo); }}
            ondragover={(e) => onTodoDragOver(e, todo)}
            ondragleave={onTodoDragLeave}
            ondrop={(e) => { if (dragTodo) { e.stopPropagation(); onTodoDrop(e, group.project); } }}
            ondragend={cleanTodoDrag}
          >
            <button
              class="todo-checkbox"
              title={$trad("todos.markDone")}
              aria-label={$trad("todos.markDone")}
              onclick={(e) => { e.stopPropagation(); toggleDone(todo); }}
              ondragstart={(e) => e.preventDefault()}
            ></button>
            {#if editingTodoId === todo.id}
              <InlineEdit
                multiligne
                value={todo.text}
                onCommit={(v) => commitEditTodo(todo, v)}
                onCancel={cancelEditTodo}
              />
            {:else}
              <TodoText texte={todo.text} dense sansGlisser onEdit={() => startEditTodo(todo)} />
            {/if}
            <!-- Le tableau de bord est l'endroit ou la demande prend son sens : voir d'un coup
                 lesquelles sont en cours et ou elles en sont. -->
            <TodoProgress
              valeur={todo.progress}
              dense
              onChange={(v) => commitProgress(todo, v)}
              onInteraction={(actif) => (reglageEnCours = actif ? todo.id : null)}
            />
            {#if editingDueId === todo.id}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="due-input"
                type="date"
                value={todo.due_date ?? ""}
                autofocus
                onclick={(e) => e.stopPropagation()}
                ondragstart={(e) => e.preventDefault()}
                onchange={(e) => commitDue(todo, e.currentTarget.value)}
                onblur={() => (editingDueId = null)}
                onkeydown={(e) => e.key === "Escape" && (editingDueId = null)}
              />
            {:else if todo.due_date}
              <button
                class="due-badge {dueUrgency(todo.due_date)}"
                title={$trad("todos.dueHint", { date: todo.due_date })}
                onclick={(e) => { e.stopPropagation(); editingDueId = todo.id; }}
                ondragstart={(e) => e.preventDefault()}
              >{dueLabel(todo.due_date)}</button>
            {:else}
              <button
                class="due-add"
                title={$trad("todos.addDue")}
                aria-label={$trad("todos.addDue")}
                onclick={(e) => { e.stopPropagation(); editingDueId = todo.id; }}
                ondragstart={(e) => e.preventDefault()}
              >📅</button>
            {/if}
            <button
              class="todo-del"
              title={$trad("common.delete")}
              aria-label={$trad("common.delete")}
              onclick={(e) => { e.stopPropagation(); removeTodo(todo); }}
              ondragstart={(e) => e.preventDefault()}
            >×</button>
          </div>
        {/each}
      </div>
    {/each}
    {#if groupedTodos.length === 0}
      <p class="empty">{$trad("tasks.empty")}</p>
    {/if}
  </div>
</div>

<style>
  .todos-panel {
    flex: 1; min-width: 0;
    background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px;
    overflow: hidden;
  }
  .panel-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.75rem 1rem; border-bottom: 1px solid var(--border-color);
  }
  .panel-header h3 { font-size: 1rem; margin: 0; }
  .task-count { font-size: 0.85rem; color: var(--text-muted); }
  .todos-list { max-height: calc(100vh - 180px); overflow-y: auto; }

  .todo-group { border-bottom: 1px solid var(--border-color); }
  .todo-group:last-child { border-bottom: 1px solid transparent; }

  .group-header-wrap {
    border-radius: 4px; transition: outline 0.1s;
  }
  .group-header-wrap.drag-target-group {
    outline: 2px solid var(--accent); outline-offset: -2px;
  }
  .group-header {
    display: flex; justify-content: space-between; align-items: center;
    width: 100%; padding: 0.6rem 1rem; background: none; border: none;
    color: var(--text-primary); font-weight: 600; font-size: 0.9rem;
    cursor: pointer; text-align: left;
  }
  .group-header:hover { background: var(--bg-tertiary); }
  .group-header.liste { cursor: default; font-style: italic; gap: 0.5rem; }
  .group-header.liste:hover { background: none; }
  .group-header.liste .group-name { flex: 1; }
  /* Toujours visibles, en retrait : caches jusqu'au survol, on ne devine pas qu'une liste se
     renomme, et c'est justement ce qu'on vient chercher. */
  .liste-actions { display: flex; gap: 0.25rem; opacity: 0.55; transition: opacity 0.15s; }
  .group-header.liste:hover .liste-actions, .liste-actions:focus-within { opacity: 1; }
  .liste-btn {
    background: none; border: 1px solid var(--border-color); border-radius: 4px; cursor: pointer;
    color: var(--text-secondary); font-size: 0.8rem; line-height: 1; padding: 0.15rem 0.35rem; font-style: normal;
  }
  .liste-btn:hover { color: var(--text-primary); border-color: var(--text-secondary); }
  .liste-btn.danger:hover { color: var(--error); border-color: var(--error); }
  .liste-vide { margin: 0; padding: 0.2rem 1rem 0.5rem 1.5rem; font-size: 0.8rem; color: var(--text-muted); }
  .saisie { display: flex; gap: 0.5rem; align-items: flex-start; padding: 0.6rem 1rem; border-bottom: 1px solid var(--border-color); }
  .saisie-champ { flex: 1; min-width: 0; }
  .saisie-cible, .nouvelle-liste {
    font-size: 0.8rem; padding: 0.35rem 0.5rem; border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--surface-base); color: var(--text-primary);
  }
  .saisie-cible { max-width: 12rem; }
  .nouvelle-liste { cursor: pointer; white-space: nowrap; }
  .nouvelle-liste:hover { border-color: var(--accent); color: var(--accent); }
  .group-count {
    background: var(--accent); color: white; font-size: 0.75rem;
    padding: 0.1rem 0.5rem; border-radius: 10px; font-weight: 600;
  }
  .todo-item {
    display: flex; align-items: flex-start; gap: 0.5rem;
    padding: 0.35rem 1rem 0.35rem 1.5rem; font-size: 0.85rem;
    color: var(--text-secondary); cursor: grab;
  }
  .todo-checkbox {
    flex-shrink: 0;
    width: 18px; height: 18px;
    border: 2px solid var(--text-muted);
    background: transparent;
    border-radius: 50%;
    padding: 0; margin: 0.1rem 0 0;
    cursor: pointer;
    transition: all 0.15s;
    position: relative;
    display: inline-block;
  }
  .todo-checkbox:hover {
    border-color: var(--success);
    background: rgba(34, 197, 94, 0.1);
  }
  .todo-checkbox:hover::after {
    content: "";
    position: absolute;
    top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    width: 8px; height: 8px;
    border-radius: 50%;
    background: var(--success);
  }
  .due-badge {
    flex-shrink: 0; border: 1px solid var(--border-color); border-radius: 10px;
    background: var(--bg-tertiary); color: var(--text-secondary);
    font-size: 0.7rem; padding: 0.08rem 0.45rem; white-space: nowrap; cursor: pointer;
  }
  .due-badge:hover { border-color: var(--text-secondary); color: var(--text-primary); }
  .due-badge.today { border-color: var(--warning); color: var(--warning); }
  .due-badge.overdue { border-color: var(--error); color: var(--error); }
  /* Le 📅 n'apparait qu'au survol de la ligne, comme la croix de suppression. */
  .due-add {
    background: none; border: none; cursor: pointer; padding: 0; font-size: 0.8rem;
    opacity: 0; flex-shrink: 0; transition: opacity 0.15s;
  }
  .todo-item:hover .due-add { opacity: 0.7; }
  .due-add:hover { opacity: 1; }
  .due-input {
    flex-shrink: 0; font-size: 0.75rem; padding: 0.1rem 0.3rem;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary);
  }
  .todo-del {
    flex-shrink: 0; background: none; border: none;
    color: var(--error); font-size: 1.1rem; padding: 0; cursor: pointer;
    opacity: 0; transition: opacity 0.15s;
    line-height: 1;
  }
  .todo-item:hover .todo-del { opacity: 1; }
</style>
