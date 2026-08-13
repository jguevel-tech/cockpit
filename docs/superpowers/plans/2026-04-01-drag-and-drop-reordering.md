# Drag-and-Drop Reordering — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add drag-and-drop reordering to projects (sidebar), todos, and notes (folders + files with cross-folder move).

**Architecture:** HTML5 native drag-and-drop on the frontend (zero dependencies). Backend already has `reorder_projects` and `reorder_todos`; we add 3 new Rust functions for notes (reorder folders, reorder files, move file). Each frontend component gets drag handlers + visual indicators.

**Tech Stack:** Rust/rusqlite (backend), Svelte 5 runes + HTML5 DnD API (frontend)

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/storage/notes.rs` | Modify | +3 DB functions: `reorder_note_folders`, `reorder_note_files`, `move_note_file` |
| `src-tauri/src/lib.rs` | Modify | +3 Tauri commands, register in invoke_handler |
| `src/lib/api/storage.ts` | Modify | +3 invoke wrappers |
| `src/lib/components/layout/Sidebar.svelte` | Modify | +drag-and-drop reordering for projects |
| `src/lib/components/todos/TodoList.svelte` | Modify | +drag-and-drop reordering for todos |
| `src/lib/components/notes/NoteTree.svelte` | Modify | +drag-and-drop reordering for folders/files + cross-folder move |

---

### Task 1: Backend — reorder_note_folders + reorder_note_files

**Files:**
- Modify: `src-tauri/src/storage/notes.rs`

- [ ] **Step 1: Write failing test for reorder_note_folders**

Add this test at the end of the `tests` module in `src-tauri/src/storage/notes.rs`:

```rust
#[test]
fn test_reorder_note_folders() {
    let db = Database::new(":memory:").unwrap();
    let a = db.create_note_folder("proj", None, "Alpha").unwrap();
    let b = db.create_note_folder("proj", None, "Beta").unwrap();
    let c = db.create_note_folder("proj", None, "Gamma").unwrap();

    // Reorder: Gamma, Alpha, Beta
    db.reorder_note_folders(&[c.id, a.id, b.id]).unwrap();

    let tree = db.get_note_tree("proj").unwrap();
    assert_eq!(tree.folders[0].name, "Gamma");
    assert_eq!(tree.folders[1].name, "Alpha");
    assert_eq!(tree.folders[2].name, "Beta");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_reorder_note_folders -- --nocapture`
Expected: compilation error — `reorder_note_folders` method not found

- [ ] **Step 3: Implement reorder_note_folders**

Add this method to the `impl Database` block in `src-tauri/src/storage/notes.rs`, after the `delete_note_file` method (before the closing `}`):

```rust
pub fn reorder_note_folders(&self, ids: &[i64]) -> Result<(), String> {
    let conn = self.conn();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE note_folders SET position=?1 WHERE id=?2",
            rusqlite::params![i as i32, id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test test_reorder_note_folders -- --nocapture`
Expected: PASS

- [ ] **Step 5: Write failing test for reorder_note_files**

Add this test in the same `tests` module:

```rust
#[test]
fn test_reorder_note_files() {
    let db = Database::new(":memory:").unwrap();
    let folder = db.create_note_folder("proj", None, "Docs").unwrap();
    let a = db.create_note_file("proj", Some(folder.id), "A.md").unwrap();
    let b = db.create_note_file("proj", Some(folder.id), "B.md").unwrap();
    let c = db.create_note_file("proj", Some(folder.id), "C.md").unwrap();

    // Reorder: C, A, B
    db.reorder_note_files(&[c.id, a.id, b.id]).unwrap();

    let tree = db.get_note_tree("proj").unwrap();
    let folder_files: Vec<_> = tree.files.iter().filter(|f| f.folder_id == Some(folder.id)).collect();
    assert_eq!(folder_files[0].name, "C.md");
    assert_eq!(folder_files[1].name, "A.md");
    assert_eq!(folder_files[2].name, "B.md");
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_reorder_note_files -- --nocapture`
Expected: compilation error — `reorder_note_files` method not found

- [ ] **Step 7: Implement reorder_note_files**

Add this method right after `reorder_note_folders` in the `impl Database` block:

```rust
pub fn reorder_note_files(&self, ids: &[i64]) -> Result<(), String> {
    let conn = self.conn();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE note_files SET position=?1 WHERE id=?2",
            rusqlite::params![i as i32, id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}
```

- [ ] **Step 8: Run test to verify it passes**

Run: `cd src-tauri && cargo test test_reorder_note_files -- --nocapture`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/storage/notes.rs
git commit -m "feat: add reorder_note_folders and reorder_note_files"
```

---

### Task 2: Backend — move_note_file

**Files:**
- Modify: `src-tauri/src/storage/notes.rs`

- [ ] **Step 1: Write failing test for move_note_file**

Add this test in the `tests` module:

```rust
#[test]
fn test_move_note_file() {
    let db = Database::new(":memory:").unwrap();
    let folder_a = db.create_note_folder("proj", None, "FolderA").unwrap();
    let folder_b = db.create_note_folder("proj", None, "FolderB").unwrap();
    let file = db.create_note_file("proj", Some(folder_a.id), "notes.md").unwrap();

    // Move file from FolderA to FolderB
    db.move_note_file(file.id, Some(folder_b.id)).unwrap();
    let moved = db.get_note_file(file.id).unwrap();
    assert_eq!(moved.folder_id, Some(folder_b.id));

    // Move file to root (no folder)
    db.move_note_file(file.id, None).unwrap();
    let at_root = db.get_note_file(file.id).unwrap();
    assert_eq!(at_root.folder_id, None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test test_move_note_file -- --nocapture`
Expected: compilation error — `move_note_file` method not found

- [ ] **Step 3: Implement move_note_file**

Add this method right after `reorder_note_files` in the `impl Database` block:

```rust
pub fn move_note_file(&self, id: i64, folder_id: Option<i64>) -> Result<(), String> {
    let conn = self.conn();
    // Get max position in target folder
    let max_pos: i32 = match folder_id {
        Some(fid) => conn.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM note_files WHERE folder_id=?1",
            [fid],
            |row| row.get(0),
        ),
        None => conn.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM note_files WHERE folder_id IS NULL",
            [],
            |row| row.get(0),
        ),
    }
    .unwrap_or(-1);

    conn.execute(
        "UPDATE note_files SET folder_id=?1, position=?2 WHERE id=?3",
        rusqlite::params![folder_id, max_pos + 1, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd src-tauri && cargo test test_move_note_file -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run all tests to ensure no regressions**

Run: `cd src-tauri && cargo test`
Expected: All 25 tests pass (22 existing + 3 new)

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/storage/notes.rs
git commit -m "feat: add move_note_file for cross-folder drag"
```

---

### Task 3: Backend — Tauri commands + Frontend API wrappers

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api/storage.ts`

- [ ] **Step 1: Add 3 Tauri commands to lib.rs**

In `src-tauri/src/lib.rs`, add these 3 commands after the `delete_note_file` command (after line 132):

```rust
#[tauri::command]
fn reorder_note_folders(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_note_folders(&ids)
}

#[tauri::command]
fn reorder_note_files(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_note_files(&ids)
}

#[tauri::command]
fn move_note_file(id: i64, folder_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.move_note_file(id, folder_id)
}
```

- [ ] **Step 2: Register commands in invoke_handler**

In the `.invoke_handler(tauri::generate_handler![...])` block, add the 3 new commands after `delete_note_file,` (after line 491):

```rust
            reorder_note_folders,
            reorder_note_files,
            move_note_file,
```

- [ ] **Step 3: Verify Rust compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles without errors

- [ ] **Step 4: Add 3 API wrappers to storage.ts**

In `src/lib/api/storage.ts`, add these lines at the end of the `// Notes` section (after line 25):

```typescript
export const reorderNoteFolders = (ids: number[]) => invoke("reorder_note_folders", { ids });
export const reorderNoteFiles = (ids: number[]) => invoke("reorder_note_files", { ids });
export const moveNoteFile = (id: number, folderId: number | null) => invoke("move_note_file", { id, folderId });
```

- [ ] **Step 5: Verify frontend compiles**

Run: `npm run build`
Expected: builds without errors

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs src/lib/api/storage.ts
git commit -m "feat: wire up reorder/move note commands through Tauri IPC"
```

---

### Task 4: Frontend — Sidebar drag-and-drop (projects)

**Files:**
- Modify: `src/lib/components/layout/Sidebar.svelte`

- [ ] **Step 1: Rewrite Sidebar.svelte with drag-and-drop**

Replace the full content of `src/lib/components/layout/Sidebar.svelte` with:

```svelte
<script lang="ts">
  import { projects } from "../../stores/projects";
  import { selectedProject, selectProject } from "../../stores/ui";
  import { reorderProjects } from "../../api/scanner";
  import { loadProjects } from "../../stores/projects";
  import type { Project } from "../../types";

  const stateColors: Record<string, string> = {
    running: "var(--success)",
    starting: "var(--warning)",
    stopping: "var(--warning)",
    error: "var(--error)",
    stopped: "var(--text-muted)",
  };

  function getColor(state: string) { return stateColors[state] || "var(--text-muted)"; }

  // Drag state
  let dragIndex: number | null = $state(null);
  let dropTarget: { index: number; pos: "before" | "after" } | null = $state(null);

  function onDragStart(e: DragEvent, index: number) {
    dragIndex = index;
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", String(index));
  }

  function onDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    if (dragIndex === null || dragIndex === index) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos = e.clientY < rect.top + rect.height / 2 ? "before" : "after";
    dropTarget = { index, pos };
  }

  function onDragLeave() {
    dropTarget = null;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    if (dragIndex === null || dropTarget === null) return;

    const items = [...$projects];
    const [moved] = items.splice(dragIndex, 1);
    let targetIdx = dropTarget.index;
    if (dragIndex < targetIdx) targetIdx--;
    if (dropTarget.pos === "after") targetIdx++;
    items.splice(targetIdx, 0, moved);

    projects.set(items);
    reorderProjects(items.map(p => p.name)).catch(() => loadProjects());

    dragIndex = null;
    dropTarget = null;
  }

  function onDragEnd() {
    dragIndex = null;
    dropTarget = null;
  }
</script>

<aside>
  <div class="sidebar-header">Projets</div>
  <ul>
    {#each $projects as proj, i}
      <li
        draggable="true"
        ondragstart={(e) => onDragStart(e, i)}
        ondragover={(e) => onDragOver(e, i)}
        ondragleave={onDragLeave}
        ondrop={onDrop}
        ondragend={onDragEnd}
        class:dragging={dragIndex === i}
        class:drag-over-top={dropTarget?.index === i && dropTarget?.pos === "before"}
        class:drag-over-bottom={dropTarget?.index === i && dropTarget?.pos === "after"}
      >
        <button
          class="project-item"
          class:active={$selectedProject === proj.name}
          onclick={() => selectProject(proj.name)}
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
              <span class="container-count">{proj.containers.length} containers</span>
            {/if}
          </div>
        </button>
      </li>
    {/each}
    {#if $projects.length === 0}
      <li class="empty">Aucun projet</li>
    {/if}
  </ul>
</aside>

<style>
  aside {
    width: var(--sidebar-width); min-width: var(--sidebar-width);
    background: var(--bg-secondary); border-right: 1px solid var(--border-color);
    overflow-y: auto; display: flex; flex-direction: column;
  }
  .sidebar-header {
    padding: 0.75rem 1rem; font-weight: 600; font-size: 0.85rem;
    color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em;
  }
  ul { list-style: none; padding: 0; margin: 0; }
  li { transition: border-color 0.1s; border-top: 2px solid transparent; border-bottom: 2px solid transparent; }
  li.dragging { opacity: 0.4; }
  li.drag-over-top { border-top-color: var(--accent); }
  li.drag-over-bottom { border-bottom-color: var(--accent); }
  .project-item {
    display: flex; flex-direction: column; gap: 0.2rem;
    width: 100%; padding: 0.6rem 1rem; border: none; background: none;
    color: var(--text-primary); cursor: pointer; text-align: left;
    border-bottom: 1px solid var(--border-color);
  }
  .project-item:hover { background: var(--bg-tertiary); }
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
</style>
```

- [ ] **Step 2: Verify frontend compiles**

Run: `npm run build`
Expected: builds without errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/layout/Sidebar.svelte
git commit -m "feat: drag-and-drop reordering for projects in sidebar"
```

---

### Task 5: Frontend — TodoList drag-and-drop

**Files:**
- Modify: `src/lib/components/todos/TodoList.svelte`

- [ ] **Step 1: Rewrite TodoList.svelte with drag-and-drop**

Replace the full content of `src/lib/components/todos/TodoList.svelte` with:

```svelte
<script lang="ts">
  import { getTodos, createTodo, updateTodo, deleteTodo, reorderTodos } from "../../api/storage";
  import type { Todo } from "../../types";
  import { onMount } from "svelte";

  let { project }: { project: string } = $props();

  let todos: Todo[] = $state([]);
  let newText = $state("");

  onMount(() => load());

  async function load() {
    try { todos = await getTodos(project); } catch {}
  }

  async function add() {
    if (!newText.trim()) return;
    try { await createTodo(project, newText.trim()); newText = ""; await load(); } catch {}
  }

  async function toggle(t: Todo) {
    try { await updateTodo(t.id, t.text, !t.done); await load(); } catch {}
  }

  async function remove(id: number) {
    try { await deleteTodo(id); await load(); } catch {}
  }

  function onKeydown(e: KeyboardEvent) { if (e.key === "Enter") add(); }

  // Drag state
  let dragIndex: number | null = $state(null);
  let dropTarget: { index: number; pos: "before" | "after" } | null = $state(null);

  function onDragStart(e: DragEvent, index: number) {
    dragIndex = index;
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", String(index));
  }

  function onDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    if (dragIndex === null || dragIndex === index) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos = e.clientY < rect.top + rect.height / 2 ? "before" : "after";
    dropTarget = { index, pos };
  }

  function onDragLeave() {
    dropTarget = null;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    if (dragIndex === null || dropTarget === null) return;

    const items = [...todos];
    const [moved] = items.splice(dragIndex, 1);
    let targetIdx = dropTarget.index;
    if (dragIndex < targetIdx) targetIdx--;
    if (dropTarget.pos === "after") targetIdx++;
    items.splice(targetIdx, 0, moved);

    todos = items;
    reorderTodos(items.map(t => t.id)).catch(() => load());

    dragIndex = null;
    dropTarget = null;
  }

  function onDragEnd() {
    dragIndex = null;
    dropTarget = null;
  }
</script>

<div class="todo-list">
  <h3>Todos</h3>
  <div class="add-row">
    <input type="text" bind:value={newText} placeholder="Nouvelle tache..." onkeydown={onKeydown} />
    <button onclick={add}>+</button>
  </div>
  <ul>
    {#each todos as todo, i}
      <li
        class:done={todo.done}
        draggable="true"
        ondragstart={(e) => onDragStart(e, i)}
        ondragover={(e) => onDragOver(e, i)}
        ondragleave={onDragLeave}
        ondrop={onDrop}
        ondragend={onDragEnd}
        class:dragging={dragIndex === i}
        class:drag-over-top={dropTarget?.index === i && dropTarget?.pos === "before"}
        class:drag-over-bottom={dropTarget?.index === i && dropTarget?.pos === "after"}
      >
        <button class="check" onclick={() => toggle(todo)}>{todo.done ? '✓' : '○'}</button>
        <span class="text">{todo.text}</span>
        <button class="del" onclick={() => remove(todo.id)}>×</button>
      </li>
    {/each}
    {#if todos.length === 0}
      <li class="empty">Aucune tache</li>
    {/if}
  </ul>
</div>

<style>
  .todo-list { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }
  h3 { margin: 0 0 0.75rem; font-size: 0.95rem; }
  .add-row { display: flex; gap: 0.5rem; margin-bottom: 0.5rem; }
  .add-row input { flex: 1; padding: 0.35rem 0.5rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.85rem; }
  .add-row button { padding: 0.35rem 0.6rem; border: 1px solid var(--border-color); border-radius: 6px; background: var(--accent); color: white; cursor: pointer; font-size: 1rem; }
  ul { list-style: none; padding: 0; margin: 0; }
  li {
    display: flex; align-items: center; gap: 0.5rem; padding: 0.3rem 0; font-size: 0.85rem;
    border-top: 2px solid transparent; border-bottom: 2px solid transparent; transition: border-color 0.1s;
  }
  li.dragging { opacity: 0.4; }
  li.drag-over-top { border-top-color: var(--accent); }
  li.drag-over-bottom { border-bottom-color: var(--accent); }
  li.done .text { text-decoration: line-through; color: var(--text-muted); }
  .check { background: none; border: none; cursor: pointer; font-size: 1rem; color: var(--text-secondary); padding: 0; }
  .text { flex: 1; }
  .del { background: none; border: none; cursor: pointer; color: var(--error); font-size: 1.1rem; padding: 0; opacity: 0; }
  li:hover .del { opacity: 1; }
  .empty { color: var(--text-muted); }
</style>
```

- [ ] **Step 2: Verify frontend compiles**

Run: `npm run build`
Expected: builds without errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/todos/TodoList.svelte
git commit -m "feat: drag-and-drop reordering for todos"
```

---

### Task 6: Frontend — NoteTree drag-and-drop (folders + files + move)

**Files:**
- Modify: `src/lib/components/notes/NoteTree.svelte`

- [ ] **Step 1: Rewrite NoteTree.svelte with drag-and-drop**

Replace the full content of `src/lib/components/notes/NoteTree.svelte` with:

```svelte
<script lang="ts">
  import { getNoteTree, createNoteFolder, createNoteFile, deleteNoteFolder, deleteNoteFile, getNoteFile, saveNoteFile, reorderNoteFolders, reorderNoteFiles, moveNoteFile } from "../../api/storage";
  import type { NoteTree as NoteTreeType, NoteFile, NoteFolder } from "../../types";
  import { onMount } from "svelte";
  import { marked } from "marked";
  import TurndownService from "turndown";

  let { project }: { project: string } = $props();

  const turndown = new TurndownService({ headingStyle: "atx", codeBlockStyle: "fenced" });

  let tree: NoteTreeType = $state({ folders: [], files: [] });
  let selectedFile: NoteFile | null = $state(null);
  let markdownContent = $state("");
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let editorEl: HTMLDivElement | undefined = $state(undefined);

  onMount(() => loadTree());

  async function loadTree() {
    try { tree = await getNoteTree(project); } catch {}
  }

  async function addFolder() {
    const name = prompt("Nom du dossier :");
    if (!name) return;
    try { await createNoteFolder(project, null, name); await loadTree(); } catch {}
  }

  async function addFile(folderId: number | null) {
    const name = prompt("Nom du fichier :");
    if (!name) return;
    try { await createNoteFile(project, folderId, name); await loadTree(); } catch {}
  }

  async function openFile(id: number) {
    try {
      selectedFile = await getNoteFile(id);
      markdownContent = selectedFile?.content || "";
      requestAnimationFrame(() => {
        if (editorEl) {
          editorEl.innerHTML = marked.parse(markdownContent) as string;
        }
      });
    } catch {}
  }

  function onEditorInput() {
    if (!editorEl) return;
    markdownContent = turndown.turndown(editorEl.innerHTML);
    scheduleSave();
  }

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      if (selectedFile) {
        try { await saveNoteFile(selectedFile.id, markdownContent); } catch {}
      }
    }, 1000);
  }

  async function removeFolder(id: number) {
    if (!confirm("Supprimer ce dossier et son contenu ?")) return;
    try { await deleteNoteFolder(id); await loadTree(); } catch {}
  }

  async function removeFile(id: number) {
    try { await deleteNoteFile(id); if (selectedFile?.id === id) selectedFile = null; await loadTree(); } catch {}
  }

  function filesInFolder(folderId: number | null): NoteFile[] {
    return tree.files.filter(f => f.folder_id === folderId);
  }

  // Toolbar
  function format(cmd: string, value: string = "") {
    document.execCommand(cmd, false, value);
    editorEl?.focus();
    onEditorInput();
  }

  function insertHeading(level: number) {
    document.execCommand("formatBlock", false, `h${level}`);
    editorEl?.focus();
    onEditorInput();
  }

  // --- Drag & Drop ---
  type DragItem = { type: "folder"; id: number } | { type: "file"; id: number; folderId: number | null };

  let dragItem: DragItem | null = $state(null);

  // Folder drag state
  let folderDropTarget: { index: number; pos: "before" | "after" } | null = $state(null);

  // File drag state (per-folder context)
  let fileDropTarget: { id: number; pos: "before" | "after" } | null = $state(null);

  // Folder as move target (file dropped onto folder header)
  let folderMoveTarget: number | null = $state(null);

  // Root drop zone target (for moving file to root)
  let rootDropActive = $state(false);

  function onFolderDragStart(e: DragEvent, folder: NoteFolder, index: number) {
    dragItem = { type: "folder", id: folder.id };
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", JSON.stringify(dragItem));
  }

  function onFolderDragOver(e: DragEvent, index: number) {
    e.preventDefault();
    if (!dragItem || dragItem.type !== "folder") return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos = e.clientY < rect.top + rect.height / 2 ? "before" : "after";
    folderDropTarget = { index, pos };
  }

  function onFolderDragLeave() {
    folderDropTarget = null;
  }

  function onFolderDrop(e: DragEvent) {
    e.preventDefault();
    if (!dragItem || dragItem.type !== "folder" || !folderDropTarget) {
      cleanDragState();
      return;
    }

    const folders = [...tree.folders];
    const srcIdx = folders.findIndex(f => f.id === dragItem!.id);
    if (srcIdx === -1) { cleanDragState(); return; }

    const [moved] = folders.splice(srcIdx, 1);
    let targetIdx = folderDropTarget.index;
    if (srcIdx < targetIdx) targetIdx--;
    if (folderDropTarget.pos === "after") targetIdx++;
    folders.splice(targetIdx, 0, moved);

    tree.folders = folders;
    reorderNoteFolders(folders.map(f => f.id)).catch(() => loadTree());
    cleanDragState();
  }

  function onFileDragStart(e: DragEvent, file: NoteFile) {
    dragItem = { type: "file", id: file.id, folderId: file.folder_id };
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", JSON.stringify(dragItem));
  }

  function onFileDragOver(e: DragEvent, file: NoteFile) {
    e.preventDefault();
    if (!dragItem || dragItem.type !== "file") return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos = e.clientY < rect.top + rect.height / 2 ? "before" : "after";
    fileDropTarget = { id: file.id, pos };
  }

  function onFileDragLeave() {
    fileDropTarget = null;
  }

  function onFileDrop(e: DragEvent, contextFolderId: number | null) {
    e.preventDefault();
    if (!dragItem || dragItem.type !== "file" || !fileDropTarget) {
      cleanDragState();
      return;
    }

    const sourceFile = dragItem;

    // If moving between folders, do the move first
    if (sourceFile.folderId !== contextFolderId) {
      moveNoteFile(sourceFile.id, contextFolderId).then(() => loadTree()).catch(() => loadTree());
      cleanDragState();
      return;
    }

    // Same folder: reorder
    const folderFiles = filesInFolder(contextFolderId);
    const items = [...folderFiles];
    const srcIdx = items.findIndex(f => f.id === sourceFile.id);
    let tgtIdx = items.findIndex(f => f.id === fileDropTarget!.id);
    if (srcIdx === -1 || tgtIdx === -1) { cleanDragState(); return; }

    const [moved] = items.splice(srcIdx, 1);
    if (srcIdx < tgtIdx) tgtIdx--;
    if (fileDropTarget!.pos === "after") tgtIdx++;
    items.splice(tgtIdx, 0, moved);

    // Optimistic update
    tree.files = [
      ...tree.files.filter(f => f.folder_id !== contextFolderId),
      ...items,
    ];
    reorderNoteFiles(items.map(f => f.id)).catch(() => loadTree());
    cleanDragState();
  }

  // Dropping a file onto a folder header = move to that folder
  function onFolderHeaderDragOver(e: DragEvent, folderId: number) {
    e.preventDefault();
    if (!dragItem || dragItem.type !== "file") return;
    folderMoveTarget = folderId;
  }

  function onFolderHeaderDragLeave() {
    folderMoveTarget = null;
  }

  function onFolderHeaderDrop(e: DragEvent, folderId: number) {
    e.preventDefault();
    e.stopPropagation();
    if (!dragItem || dragItem.type !== "file") { cleanDragState(); return; }
    if (dragItem.folderId === folderId) { cleanDragState(); return; }

    moveNoteFile(dragItem.id, folderId).then(() => loadTree()).catch(() => loadTree());
    cleanDragState();
  }

  // Root drop zone: move file to root (folder_id = null)
  function onRootDragOver(e: DragEvent) {
    e.preventDefault();
    if (!dragItem || dragItem.type !== "file") return;
    rootDropActive = true;
  }

  function onRootDragLeave() {
    rootDropActive = false;
  }

  function onRootDrop(e: DragEvent) {
    e.preventDefault();
    if (!dragItem || dragItem.type !== "file") { cleanDragState(); return; }
    if (dragItem.folderId === null) { cleanDragState(); return; }

    moveNoteFile(dragItem.id, null).then(() => loadTree()).catch(() => loadTree());
    cleanDragState();
  }

  function onDragEnd() {
    cleanDragState();
  }

  function cleanDragState() {
    dragItem = null;
    folderDropTarget = null;
    fileDropTarget = null;
    folderMoveTarget = null;
    rootDropActive = false;
  }
</script>

<div class="notes">
  <div class="tree-panel">
    <div class="tree-header">
      <h3>Notes</h3>
      <button class="sm-btn" onclick={addFolder} title="Nouveau dossier">📁+</button>
      <button class="sm-btn" onclick={() => addFile(null)} title="Nouveau fichier">📄+</button>
    </div>

    {#each tree.folders as folder, fi}
      <div
        class="folder"
        class:dragging={dragItem?.type === "folder" && dragItem.id === folder.id}
        class:drag-over-top={folderDropTarget?.index === fi && folderDropTarget?.pos === "before"}
        class:drag-over-bottom={folderDropTarget?.index === fi && folderDropTarget?.pos === "after"}
        draggable="true"
        ondragstart={(e) => onFolderDragStart(e, folder, fi)}
        ondragover={(e) => onFolderDragOver(e, fi)}
        ondragleave={onFolderDragLeave}
        ondrop={onFolderDrop}
        ondragend={onDragEnd}
      >
        <div
          class="folder-header"
          class:drag-target-folder={folderMoveTarget === folder.id}
          ondragover={(e) => onFolderHeaderDragOver(e, folder.id)}
          ondragleave={onFolderHeaderDragLeave}
          ondrop={(e) => onFolderHeaderDrop(e, folder.id)}
        >
          <span>📁 {folder.name}</span>
          <button class="sm-btn" onclick={() => addFile(folder.id)}>+</button>
          <button class="sm-btn danger" onclick={() => removeFolder(folder.id)}>×</button>
        </div>
        {#each filesInFolder(folder.id) as file}
          <button
            class="file-item"
            class:active={selectedFile?.id === file.id}
            class:dragging={dragItem?.type === "file" && dragItem.id === file.id}
            class:drag-over-top={fileDropTarget?.id === file.id && fileDropTarget?.pos === "before"}
            class:drag-over-bottom={fileDropTarget?.id === file.id && fileDropTarget?.pos === "after"}
            draggable="true"
            ondragstart={(e) => onFileDragStart(e, file)}
            ondragover={(e) => onFileDragOver(e, file)}
            ondragleave={onFileDragLeave}
            ondrop={(e) => onFileDrop(e, folder.id)}
            ondragend={onDragEnd}
            onclick={() => openFile(file.id)}
          >
            📄 {file.name}
            <button class="del-btn" type="button" onclick={(e: MouseEvent) => { e.stopPropagation(); removeFile(file.id); }}>×</button>
          </button>
        {/each}
      </div>
    {/each}

    <div
      class="root-files"
      class:root-drop-active={rootDropActive}
      ondragover={onRootDragOver}
      ondragleave={onRootDragLeave}
      ondrop={onRootDrop}
    >
      {#each filesInFolder(null) as file}
        <button
          class="file-item"
          class:active={selectedFile?.id === file.id}
          class:dragging={dragItem?.type === "file" && dragItem.id === file.id}
          class:drag-over-top={fileDropTarget?.id === file.id && fileDropTarget?.pos === "before"}
          class:drag-over-bottom={fileDropTarget?.id === file.id && fileDropTarget?.pos === "after"}
          draggable="true"
          ondragstart={(e) => onFileDragStart(e, file)}
          ondragover={(e) => onFileDragOver(e, file)}
          ondragleave={onFileDragLeave}
          ondrop={(e) => onFileDrop(e, null)}
          ondragend={onDragEnd}
          onclick={() => openFile(file.id)}
        >
          📄 {file.name}
          <button class="del-btn" type="button" onclick={(e: MouseEvent) => { e.stopPropagation(); removeFile(file.id); }}>×</button>
        </button>
      {/each}
    </div>
  </div>

  {#if selectedFile}
    <div class="editor-panel">
      <div class="editor-header">
        <span class="file-title">{selectedFile.name}</span>
        <div class="toolbar">
          <button class="tb" onclick={() => format("bold")} title="Gras"><b>G</b></button>
          <button class="tb" onclick={() => format("italic")} title="Italique"><i>I</i></button>
          <button class="tb" onclick={() => format("strikeThrough")} title="Barre"><s>S</s></button>
          <span class="tb-sep"></span>
          <button class="tb" onclick={() => insertHeading(1)} title="Titre 1">H1</button>
          <button class="tb" onclick={() => insertHeading(2)} title="Titre 2">H2</button>
          <button class="tb" onclick={() => insertHeading(3)} title="Titre 3">H3</button>
          <span class="tb-sep"></span>
          <button class="tb" onclick={() => format("insertUnorderedList")} title="Liste">•</button>
          <button class="tb" onclick={() => format("insertOrderedList")} title="Liste numerotee">1.</button>
          <button class="tb" onclick={() => format("formatBlock", "blockquote")} title="Citation">❝</button>
          <span class="tb-sep"></span>
          <button class="tb" onclick={() => format("formatBlock", "pre")} title="Bloc de code">&lt;/&gt;</button>
          <button class="tb" onclick={() => { const url = prompt("URL :"); if (url) format("createLink", url); }} title="Lien">🔗</button>
        </div>
      </div>

      <div
        bind:this={editorEl}
        class="editor"
        contenteditable="true"
        oninput={onEditorInput}
      ></div>
    </div>
  {:else}
    <div class="editor-panel empty-editor">Selectionnez un fichier</div>
  {/if}
</div>

<style>
  .notes { display: flex; gap: 1rem; height: 100%; min-height: 400px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }
  .tree-panel { width: 200px; min-width: 200px; overflow-y: auto; border-right: 1px solid var(--border-color); padding-right: 0.75rem; }
  .tree-header { display: flex; align-items: center; gap: 0.25rem; margin-bottom: 0.5rem; }
  .tree-header h3 { flex: 1; margin: 0; font-size: 0.95rem; }
  .sm-btn { background: none; border: none; cursor: pointer; font-size: 0.8rem; padding: 0.1rem 0.3rem; color: var(--text-secondary); }
  .sm-btn:hover { color: var(--text-primary); }
  .sm-btn.danger:hover { color: var(--error); }

  /* Folders */
  .folder { margin-bottom: 0.5rem; border-top: 2px solid transparent; border-bottom: 2px solid transparent; transition: border-color 0.1s; }
  .folder.dragging { opacity: 0.4; }
  .folder.drag-over-top { border-top-color: var(--accent); }
  .folder.drag-over-bottom { border-bottom-color: var(--accent); }
  .folder-header { display: flex; align-items: center; gap: 0.25rem; font-size: 0.85rem; font-weight: 600; padding: 0.2rem 0; border-radius: 4px; transition: outline 0.1s; }
  .folder-header span { flex: 1; }
  .folder-header.drag-target-folder { outline: 2px solid var(--accent); outline-offset: -2px; }

  /* Files */
  .file-item {
    display: flex; align-items: center; width: 100%; padding: 0.25rem 0.5rem; border: none;
    background: none; cursor: pointer; font-size: 0.85rem; color: var(--text-primary); text-align: left; border-radius: 4px;
    border-top: 2px solid transparent; border-bottom: 2px solid transparent; transition: border-color 0.1s;
  }
  .file-item:hover { background: var(--bg-tertiary); }
  .file-item.active { background: var(--bg-tertiary); color: var(--accent); }
  .file-item.dragging { opacity: 0.4; }
  .file-item.drag-over-top { border-top-color: var(--accent); }
  .file-item.drag-over-bottom { border-bottom-color: var(--accent); }
  .del-btn { margin-left: auto; background: none; border: none; cursor: pointer; color: var(--error); opacity: 0; font-size: 0.9rem; }
  .file-item:hover .del-btn { opacity: 1; }

  /* Root drop zone */
  .root-files { min-height: 1rem; border-radius: 4px; transition: outline 0.1s; }
  .root-files.root-drop-active { outline: 2px dashed var(--accent); outline-offset: -2px; }

  /* Editor */
  .editor-panel { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .editor-header { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem; flex-wrap: wrap; }
  .file-title { font-weight: 600; font-size: 0.9rem; }

  /* Toolbar */
  .toolbar { display: flex; align-items: center; gap: 0.15rem; margin-left: auto; flex-wrap: wrap; }
  .tb {
    background: var(--bg-tertiary); border: 1px solid var(--border-color); color: var(--text-secondary);
    padding: 0.2rem 0.45rem; border-radius: 4px; cursor: pointer; font-size: 0.75rem;
    line-height: 1; min-width: 24px; text-align: center;
  }
  .tb:hover { background: var(--accent); color: white; border-color: var(--accent); }
  .tb-sep { width: 1px; height: 16px; background: var(--border-color); margin: 0 0.2rem; }

  /* Contenteditable editor */
  .editor {
    flex: 1; overflow-y: auto; padding: 0.75rem; border: 1px solid var(--border-color);
    border-radius: 6px; background: var(--bg-primary); font-size: 0.9rem; line-height: 1.6;
    outline: none; cursor: text;
  }
  .editor:focus { border-color: var(--accent); }
  .editor :global(h1) { font-size: 1.5rem; margin: 0.5rem 0; border-bottom: 1px solid var(--border-color); padding-bottom: 0.3rem; }
  .editor :global(h2) { font-size: 1.25rem; margin: 0.5rem 0; }
  .editor :global(h3) { font-size: 1.1rem; margin: 0.5rem 0; }
  .editor :global(p) { margin: 0.4rem 0; }
  .editor :global(ul), .editor :global(ol) { padding-left: 1.5rem; margin: 0.4rem 0; }
  .editor :global(code) { background: var(--bg-tertiary); padding: 0.1rem 0.3rem; border-radius: 3px; font-size: 0.85em; }
  .editor :global(pre) { background: var(--bg-tertiary); padding: 0.75rem; border-radius: 6px; overflow-x: auto; font-family: monospace; }
  .editor :global(pre code) { background: none; padding: 0; }
  .editor :global(blockquote) { border-left: 3px solid var(--accent); padding-left: 0.75rem; color: var(--text-secondary); margin: 0.4rem 0; }
  .editor :global(a) { color: var(--accent); }
  .editor :global(table) { border-collapse: collapse; width: 100%; }
  .editor :global(th), .editor :global(td) { border: 1px solid var(--border-color); padding: 0.3rem 0.5rem; }
  .editor :global(img) { max-width: 100%; }

  .empty-editor { display: flex; align-items: center; justify-content: center; color: var(--text-muted); }
</style>
```

- [ ] **Step 2: Verify frontend compiles**

Run: `npm run build`
Expected: builds without errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/notes/NoteTree.svelte
git commit -m "feat: drag-and-drop reordering and cross-folder move for notes"
```

---

### Task 7: Full integration check

**Files:** None (verification only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test`
Expected: All 25 tests pass

- [ ] **Step 2: Run frontend build**

Run: `npm run build`
Expected: builds without errors

- [ ] **Step 3: Run the app in dev mode and verify manually**

Run: `npx tauri dev`

Manual test checklist:
1. **Sidebar**: drag a project up/down, verify order persists after reload
2. **Todos**: open a project workspace, drag a todo up/down, verify order persists
3. **Notes folders**: drag a folder above/below another, verify order persists
4. **Notes files (same folder)**: drag a file within a folder, verify order persists
5. **Notes files (cross-folder)**: drag a file from one folder onto another folder header, verify it moves
6. **Notes files (to root)**: drag a file from a folder to the root zone below folders, verify it moves to root
7. Visual: blue indicator line shows correctly during drag, dragged item is semi-transparent

- [ ] **Step 4: Final commit if any tweaks needed**

```bash
git add -A
git commit -m "chore: final tweaks for drag-and-drop feature"
```
