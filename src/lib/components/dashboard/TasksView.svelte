<script lang="ts">
  import { onMount } from "svelte";
  import { getPendingTodos, reorderTodos, moveTodo, updateTodo, deleteTodo } from "../../api/storage";
  import { reorderProjects } from "../../api/scanner";
  import { selectProject, activeTab } from "../../stores/ui";
  import { projects, loadProjects } from "../../stores/projects";
  import { reorderable } from "../../actions/reorderable";
  import { reorder, groupBy, type DropPosition } from "../../utils/reorder";
  import { notify } from "../../stores/toast";
  import { dueLabel, dueUrgency } from "../../utils/due";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import type { Todo } from "../../types";
  import { trad, tradN } from "../../i18n";

  let pendingTodos: Todo[] = $state([]);

  let groupedTodos: { project: string; todos: Todo[] }[] = $derived(
    groupBy(pendingTodos, (t) => t.project).map((g) => ({ project: g.key, todos: g.items }))
  );

  let totalTodoCount: number = $derived(pendingTodos.length);

  onMount(async () => {
    try { pendingTodos = await getPendingTodos(); } catch (e) { notify(String(e)); }
  });

  async function reload() {
    try { pendingTodos = await getPendingTodos(); } catch (e) { notify(String(e)); }
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

  async function removeTodo(todo: Todo) {
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

  // --- Drag & Drop des groupes (projets) via l'action partagee ---
  function onGroupReorder(from: number, to: number, pos: DropPosition) {
    const draggedProject = groupedTodos[from]?.project;
    const targetProject = groupedTodos[to]?.project;
    if (!draggedProject || !targetProject) return;

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
  <div class="todos-list">
    {#each groupedTodos as group, gi}
      <div
        class="todo-group"
        role="listitem"
        use:reorderable={{ index: gi, group: "task-groups", onDrop: onGroupReorder }}
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
          <button class="group-header" onclick={() => { selectProject(group.project); activeTab.set("workspace"); }}>
            <span class="group-name">{group.project}</span>
            <span class="group-count">{group.todos.length}</span>
          </button>
        </div>
        {#each group.todos as todo}
          <div
            class="todo-item"
            class:dragging={dragTodo?.todoId === todo.id}
            class:drag-over-top={todoDropTarget?.todoId === todo.id && todoDropTarget?.pos === "before"}
            class:drag-over-bottom={todoDropTarget?.todoId === todo.id && todoDropTarget?.pos === "after"}
            role="listitem"
            draggable="true"
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
                value={todo.text}
                onCommit={(v) => commitEditTodo(todo, v)}
                onCancel={cancelEditTodo}
              />
            {:else}
              <button
                class="todo-text-btn"
                onclick={(e) => { e.stopPropagation(); startEditTodo(todo); }}
                ondragstart={(e) => e.preventDefault()}
                title={$trad("common.clickToEdit")}
              >{todo.text}</button>
            {/if}
            {#if todo.due_date}
              <span class="due-badge {dueUrgency(todo.due_date)}" title={$trad("tasks.due", { date: todo.due_date })}>
                {dueLabel(todo.due_date)}
              </span>
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
    padding: 0; margin: 0;
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
  .todo-text-btn {
    flex: 1; min-width: 0;
    background: none; border: 1px solid transparent;
    padding: 0.1rem 0.3rem; margin: -0.1rem -0.3rem;
    text-align: left; color: inherit; cursor: text;
    border-radius: 4px; font-size: inherit; font-family: inherit; line-height: 1.4;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .todo-text-btn:hover {
    background: var(--bg-tertiary);
    border-color: var(--border-color);
  }
  .due-badge {
    flex-shrink: 0; border: 1px solid var(--border-color); border-radius: 10px;
    background: var(--bg-tertiary); color: var(--text-secondary);
    font-size: 0.7rem; padding: 0.08rem 0.45rem; white-space: nowrap;
  }
  .due-badge.today { border-color: var(--warning); color: var(--warning); }
  .due-badge.overdue { border-color: var(--error); color: var(--error); }
  .todo-del {
    flex-shrink: 0; background: none; border: none;
    color: var(--error); font-size: 1.1rem; padding: 0; cursor: pointer;
    opacity: 0; transition: opacity 0.15s;
    line-height: 1;
  }
  .todo-item:hover .todo-del { opacity: 1; }
</style>
