<script lang="ts">
  import { getTodos, createTodo, updateTodo, deleteTodo, reorderTodos, setTodoDue, setTodoProgress } from "../../api/storage";
  import { dueLabel, dueUrgency } from "../../utils/due";
  import type { Todo } from "../../types";
  import { onMount } from "svelte";
  import { reorderable } from "../../actions/reorderable";
  import { reorder, type DropPosition } from "../../utils/reorder";
  import { notify } from "../../stores/toast";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import TodoText from "./TodoText.svelte";
  import TodoProgress from "./TodoProgress.svelte";
  import { trad } from "../../i18n";
  import { demanderConfirmation } from "../../stores/confirm";

  let { project }: { project: string } = $props();

  let todos: Todo[] = $state([]);
  let newText = $state("");
  let showDone = $state(false);
  let editingId: number | null = $state(null);
  let editingDueId: number | null = $state(null);

  let pendingTodos = $derived(todos.filter(t => !t.done));
  let doneTodos = $derived(todos.filter(t => t.done));
  let visibleTodos = $derived(showDone ? doneTodos : pendingTodos);

  onMount(() => load());

  async function load() {
    try { todos = await getTodos(project); } catch (e) { notify(String(e)); }
  }

  async function add() {
    if (!newText.trim()) return;
    try { await createTodo(project, newText.trim()); newText = ""; await load(); } catch (e) { notify(String(e)); }
  }

  async function toggle(t: Todo) {
    try { await updateTodo(t.id, t.text, !t.done); await load(); } catch (e) { notify(String(e)); }
  }

  // La ligne dont le curseur d'avancement est en cours de reglage sort du glisser-deposer :
  // sinon tirer le curseur demarre un deplacement de la tache, et les deux gestes se battent.
  let reglageEnCours: number | null = $state(null);

  async function remove(t: Todo) {
    const question = $trad("todos.deleteConfirm", { texte: t.text });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    try { await deleteTodo(t.id); await load(); } catch (e) { notify(String(e)); }
  }

  async function commitEdit(t: Todo, next: string) {
    editingId = null;
    try { await updateTodo(t.id, next, t.done); await load(); } catch (e) { notify(String(e)); }
  }

  async function commitDue(t: Todo, value: string) {
    editingDueId = null;
    try { await setTodoDue(t.id, value || null); await load(); } catch (e) { notify(String(e)); }
  }

  /// 100 % marque la tache finie, cote base : la liste est donc rechargee pour qu'elle passe
  /// dans le bon groupe au lieu de rester affichee comme en cours.
  async function commitProgress(t: Todo, valeur: number) {
    try { await setTodoProgress(t.id, valeur); await load(); } catch (e) { notify(String(e)); }
  }

  function onKeydown(e: KeyboardEvent) { if (e.key === "Enter") add(); }

  function moveTodo(from: number, to: number, pos: DropPosition) {
    const items = reorder(pendingTodos, from, to, pos);
    // Recompose: pending reordonne + done inchanges
    todos = [...items, ...doneTodos];
    reorderTodos(todos.map(t => t.id)).catch((e) => { notify(String(e)); load(); });
  }
</script>

<div class="todo-list">
  <div class="header">
    <h3>{$trad("todos.title")}</h3>
    {#if doneTodos.length > 0}
      <button class="toggle-done" class:active={showDone} onclick={() => showDone = !showDone}>
        {$trad("todos.doneToggle", { count: doneTodos.length })}
      </button>
    {/if}
  </div>
  {#if !showDone}
    <div class="add-row">
      <input type="text" bind:value={newText} placeholder={$trad("todos.newPlaceholder")} onkeydown={onKeydown} />
      <button onclick={add}>+</button>
    </div>
  {/if}
  <ul>
    {#each visibleTodos as todo, i}
      <li
        class:done={todo.done}
        use:reorderable={{ index: i, group: "todos", onDrop: moveTodo, disabled: showDone || reglageEnCours === todo.id }}
      >
        <button
          class="check"
          class:checked={todo.done}
          onclick={() => toggle(todo)}
          title={todo.done ? $trad("todos.markUndone") : $trad("todos.markDone")}
          aria-label={todo.done ? $trad("todos.markUndone") : $trad("todos.markDone")}
        ></button>
        {#if editingId === todo.id}
          <InlineEdit
            value={todo.text}
            onCommit={(v) => commitEdit(todo, v)}
            onCancel={() => (editingId = null)}
          />
        {:else}
          <TodoText texte={todo.text} done={todo.done} onEdit={() => (editingId = todo.id)} />
        {/if}
        {#if !todo.done}
          <TodoProgress
            valeur={todo.progress}
            onChange={(v) => commitProgress(todo, v)}
            onInteraction={(actif) => (reglageEnCours = actif ? todo.id : null)}
          />
        {/if}
        {#if editingDueId === todo.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="due-input"
            type="date"
            value={todo.due_date ?? ""}
            autofocus
            onchange={(e) => commitDue(todo, e.currentTarget.value)}
            onblur={() => (editingDueId = null)}
            onkeydown={(e) => e.key === "Escape" && (editingDueId = null)}
          />
        {:else if todo.due_date}
          <button
            class="due-badge {dueUrgency(todo.due_date)}"
            onclick={() => (editingDueId = todo.id)}
            title={$trad("todos.dueHint", { date: todo.due_date })}
          >{dueLabel(todo.due_date)}</button>
        {:else if !todo.done}
          <button class="due-add" onclick={() => (editingDueId = todo.id)} title={$trad("todos.addDue")}>📅</button>
        {/if}
        <button class="del" onclick={() => remove(todo)} title={$trad("common.delete")}>×</button>
      </li>
    {/each}
    {#if visibleTodos.length === 0}
      <li class="empty">{showDone ? $trad("todos.emptyDone") : $trad("todos.empty")}</li>
    {/if}
  </ul>
</div>

<style>
  .todo-list { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }
  .header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 0.75rem; }
  h3 { margin: 0; font-size: 0.95rem; }
  .toggle-done {
    background: none; border: 1px solid var(--border-color); border-radius: 4px;
    padding: 0.15rem 0.5rem; font-size: 0.75rem; color: var(--text-muted);
    cursor: pointer; transition: all 0.15s;
  }
  .toggle-done:hover { color: var(--text-primary); border-color: var(--text-secondary); }
  .toggle-done.active { background: var(--accent); color: white; border-color: var(--accent); }
  .add-row { display: flex; gap: 0.5rem; margin-bottom: 0.5rem; }
  .add-row input { flex: 1; padding: 0.35rem 0.5rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.85rem; }
  .add-row button { padding: 0.35rem 0.6rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--accent); color: white; cursor: pointer; font-size: 1rem; }
  ul { list-style: none; padding: 0; margin: 0; }
  li {
    display: flex; align-items: center; gap: 0.5rem; padding: 0.3rem 0; font-size: 0.85rem;
  }
  .check {
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
  .check:hover {
    border-color: var(--success);
    background: rgba(34, 197, 94, 0.1);
  }
  .check:hover::after {
    content: "";
    position: absolute;
    top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    width: 8px; height: 8px;
    border-radius: 50%;
    background: var(--success);
  }
  .check.checked {
    border-color: var(--success);
    background: var(--success);
  }
  .check.checked::after {
    content: "✓";
    position: absolute;
    top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    color: white;
    font-size: 12px;
    font-weight: bold;
    line-height: 1;
  }
  .del { background: none; border: none; cursor: pointer; color: var(--error); font-size: 1.1rem; padding: 0; opacity: 0; flex-shrink: 0; }
  li:hover .del { opacity: 1; }
  /* Echeances : badge colore par urgence ; le 📅 n'apparait qu'au survol (comme la croix) */
  .due-badge {
    flex-shrink: 0; border: 1px solid var(--border-color); border-radius: 10px;
    background: var(--bg-tertiary); color: var(--text-secondary);
    font-size: 0.7rem; padding: 0.08rem 0.45rem; cursor: pointer; white-space: nowrap;
  }
  .due-badge.today { border-color: var(--warning); color: var(--warning); }
  .due-badge.overdue { border-color: var(--error); color: var(--error); }
  .due-add {
    background: none; border: none; cursor: pointer; padding: 0; font-size: 0.8rem;
    opacity: 0; flex-shrink: 0;
  }
  li:hover .due-add { opacity: 0.7; }
  .due-add:hover { opacity: 1 !important; }
  .due-input {
    flex-shrink: 0; font-size: 0.75rem; padding: 0.1rem 0.3rem;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary);
  }
  .empty { color: var(--text-muted); }
</style>
