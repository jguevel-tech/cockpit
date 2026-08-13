# Drag-and-Drop Reordering — Design Spec

**Date** : 2026-04-01
**Scope** : Projets (sidebar), Todos (TodoList), Notes (NoteTree)
**Approche** : HTML5 Drag & Drop natif, zero dependance

---

## 1. Perimetre

| Zone | Composant | Comportement |
|------|-----------|-------------|
| Projets | `Sidebar.svelte` | Reordonner les projets dans la sidebar |
| Todos | `TodoList.svelte` | Reordonner les taches au sein d'un projet |
| Notes (dossiers) | `NoteTree.svelte` | Reordonner les dossiers entre eux |
| Notes (fichiers) | `NoteTree.svelte` | Reordonner les fichiers + deplacer entre dossiers |

## 2. Backend — Modifications

### 2.1 Commandes existantes (aucun changement)

- `reorder_projects(names: Vec<String>)` — `storage/projects.rs`
- `reorder_todos(ids: Vec<i64>)` — `storage/todos.rs`

### 2.2 Nouvelles commandes

**`reorder_note_folders`** — `storage/notes.rs`
```rust
pub fn reorder_note_folders(&self, ids: &[i64]) -> Result<(), String>
```
- Meme pattern que `reorder_todos` : itere les IDs, met a jour `position = index`
- Wrap dans une transaction

**`reorder_note_files`** — `storage/notes.rs`
```rust
pub fn reorder_note_files(&self, ids: &[i64]) -> Result<(), String>
```
- Meme pattern, met a jour `position = index` pour chaque fichier

**`move_note_file`** — `storage/notes.rs`
```rust
pub fn move_note_file(&self, id: i64, folder_id: Option<i64>) -> Result<(), String>
```
- UPDATE `note_files SET folder_id = ?1 WHERE id = ?2`
- Met aussi `position` au MAX+1 du dossier cible

### 2.3 Commandes Tauri (lib.rs)

3 nouvelles commandes a enregistrer :

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

Ajouter au `.invoke_handler(tauri::generate_handler![...])`.

## 3. Frontend — API

### 3.1 Nouvelles fonctions (storage.ts)

```typescript
export const reorderNoteFolders = (ids: number[]) =>
  invoke("reorder_note_folders", { ids });

export const reorderNoteFiles = (ids: number[]) =>
  invoke("reorder_note_files", { ids });

export const moveNoteFile = (id: number, folderId: number | null) =>
  invoke("move_note_file", { id, folderId });
```

## 4. Frontend — Mecanique de drag commune

### 4.1 Pattern general

Chaque zone de drag suit le meme cycle :

1. **dragstart** : stocker l'index et le type de l'item dans `dataTransfer`
2. **dragover** : `preventDefault()` + calculer si le curseur est dans la moitie haute ou basse de l'element survole, afficher l'indicateur correspondant (ligne au-dessus ou en-dessous)
3. **dragleave** : retirer l'indicateur
4. **drop** : calculer le nouvel ordre, mettre a jour l'array local (optimiste), appeler le backend
5. **dragend** : nettoyer tout etat visuel residuel

### 4.2 Calcul position drop

```typescript
function getDropPosition(e: DragEvent, el: HTMLElement): "before" | "after" {
  const rect = el.getBoundingClientRect();
  const midY = rect.top + rect.height / 2;
  return e.clientY < midY ? "before" : "after";
}
```

### 4.3 Update optimiste

On reordonne l'array local immediatement au drop, puis on appelle le backend.
Si le backend echoue, on recharge la liste depuis le backend (fallback).

## 5. Frontend — Par composant

### 5.1 Sidebar.svelte (projets)

- Ajouter `draggable="true"` sur chaque `<li>` / `.project-item`
- State local : `dragIndex: number | null`, `dropTarget: { index: number, pos: "before" | "after" } | null`
- Au `drop` :
  1. Retirer l'item de sa position source
  2. L'inserer a la position cible
  3. Mettre a jour le store `projects` (optimiste)
  4. Appeler `reorderProjects(projects.map(p => p.name))`
- Import `reorderProjects` depuis `api/scanner.ts` (deja exporte)

### 5.2 TodoList.svelte (todos)

- Ajouter `draggable="true"` sur chaque `<li>` de todo
- State local : `dragIndex`, `dropTarget`
- Au `drop` :
  1. Reordonner le tableau `todos` local
  2. Appeler `reorderTodos(todos.map(t => t.id))`
- Import `reorderTodos` depuis `api/storage.ts` (deja exporte)

### 5.3 NoteTree.svelte (notes)

Plus complexe car 2 types d'items (dossiers et fichiers) et deplacement inter-dossiers.

**Dossiers** :
- `draggable="true"` sur `.folder`
- Drop entre dossiers : reordonne `tree.folders` et appelle `reorderNoteFolders(ids)`

**Fichiers** :
- `draggable="true"` sur `.file-item`
- Drop entre fichiers (meme dossier) : reordonne les fichiers localement et appelle `reorderNoteFiles(ids)`
- Drop sur un dossier (header) : highlight le dossier cible, au drop appelle `moveNoteFile(fileId, folderId)` puis recharge l'arbre

**Discrimination du type** :
- `dataTransfer.setData("text/plain", JSON.stringify({ type: "folder"|"file", id }))`
- Au drop, lire le type pour savoir quel traitement appliquer

**Fichiers a la racine** :
- Dropper un fichier dans la zone sous les dossiers = `moveNoteFile(id, null)` (retour a la racine)

## 6. Feedback visuel (CSS)

Styles ajoutes dans chaque composant (scoped via `<style>`) :

```css
/* Item en cours de drag */
.dragging {
  opacity: 0.4;
}

/* Indicateur de position de drop (ligne bleue) */
.drag-over-top {
  border-top: 2px solid var(--accent);
}
.drag-over-bottom {
  border-bottom: 2px solid var(--accent);
}

/* Dossier recepteur (NoteTree) */
.drag-target-folder {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
  border-radius: 4px;
}
```

L'item source a une opacite reduite pendant le drag.
Les autres items affichent une ligne bleue de 2px en haut ou en bas selon la position du curseur.

## 7. Pas de dependance externe

Tout est implemente avec l'API HTML5 native :
- `draggable="true"`
- Events : `ondragstart`, `ondragover`, `ondragleave`, `ondrop`, `ondragend`
- `e.dataTransfer.setData()` / `e.dataTransfer.getData()`
- `e.preventDefault()` dans `ondragover` pour autoriser le drop

## 8. Tests backend

Ajouter dans `storage/notes.rs` :
- `test_reorder_note_folders` : creer 3 dossiers, reorder, verifier positions
- `test_reorder_note_files` : creer 3 fichiers, reorder, verifier positions
- `test_move_note_file` : creer un fichier dans folder A, deplacer vers folder B, verifier folder_id

## 9. Resume des fichiers a modifier

| Fichier | Action |
|---------|--------|
| `src-tauri/src/storage/notes.rs` | +3 fonctions (reorder_folders, reorder_files, move_file) + 3 tests |
| `src-tauri/src/lib.rs` | +3 commandes Tauri, enregistrement dans invoke_handler |
| `src/lib/api/storage.ts` | +3 wrappers invoke |
| `src/lib/components/layout/Sidebar.svelte` | +drag handlers + CSS indicateurs |
| `src/lib/components/todos/TodoList.svelte` | +drag handlers + CSS indicateurs |
| `src/lib/components/notes/NoteTree.svelte` | +drag handlers (folders + files + move) + CSS indicateurs |
