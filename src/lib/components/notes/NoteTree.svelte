<script lang="ts">
  import { getNoteTree, createNoteFolder, createNoteFile, deleteNoteFolder, deleteNoteFile, getNoteFile, renameNoteFile, renameNoteFolder, reorderNoteFolders, reorderNoteFiles, moveNoteFile } from "../../api/storage";
  import type { NoteTree as NoteTreeType, NoteFile, NoteFolder } from "../../types";
  import { lastRecordingEvent } from "../../stores/recording";
  import { notify } from "../../stores/toast";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import NoteEditor from "./NoteEditor.svelte";
  import ReadingToggle from "./ReadingToggle.svelte";
  import { readingMode } from "../../stores/ui";
  import { onMount } from "svelte";
  import { trad } from "../../i18n";
  import { demanderConfirmation } from "../../stores/confirm";

  let { project }: { project: string } = $props();

  let tree: NoteTreeType = $state({ folders: [], files: [] });
  let selectedFile: NoteFile | null = $state(null);
  let notesEl: HTMLDivElement | undefined = $state(undefined);

  onMount(() => loadTree());

  // Une reunion transcrite cree une note en arriere-plan : on recharge l'arbre
  $effect(() => {
    const ev = $lastRecordingEvent;
    if (ev && ev.project === project && ev.state === "done") loadTree();
  });

  async function loadTree() {
    try { tree = await getNoteTree(project); } catch (e) { notify(String(e)); }
  }

  async function addFolder() {
    const name = prompt($trad("notes.folderNamePrompt"));
    if (!name) return;
    try { await createNoteFolder(project, null, name); await loadTree(); } catch (e) { notify(String(e)); }
  }

  async function addFile(folderId: number | null) {
    const name = prompt($trad("notes.fileNamePrompt"));
    if (!name) return;
    try { await createNoteFile(project, folderId, name); await loadTree(); } catch (e) { notify(String(e)); }
  }

  async function openFile(id: number) {
    try { selectedFile = await getNoteFile(id); } catch (e) { notify(String(e)); }
  }

  async function removeFolder(id: number) {
    if (!(await demanderConfirmation({ message: $trad("notes.deleteFolderConfirm"), action: $trad("common.delete") }))) return;
    try { await deleteNoteFolder(id); await loadTree(); } catch (e) { notify(String(e)); }
  }

  async function removeFile(id: number) {
    // Une note peut etre le compte rendu d'une reunion d'une heure : jamais de suppression
    // sur un simple clic (demande d'un utilisateur).
    const nom = tree.files.find((f) => f.id === id)?.name ?? "";
    if (!(await demanderConfirmation({ message: $trad("notes.deleteFileConfirm", { name: nom }), action: $trad("common.delete") }))) return;
    try {
      await deleteNoteFile(id);
      if (selectedFile?.id === id) selectedFile = null;
      await loadTree();
    } catch (e) {
      notify(String(e), "error", 4000, { scope: "notes.suppression" });
    }
  }

  function filesInFolder(folderId: number | null): NoteFile[] {
    return tree.files.filter(f => f.folder_id === folderId);
  }

  // --- Inline Rename ---
  let renamingItem: { type: "folder" | "file"; id: number } | null = $state(null);

  async function renameFolder(id: number, name: string) {
    renamingItem = null;
    try { await renameNoteFolder(id, name); await loadTree(); } catch (e) { notify(String(e)); }
  }

  async function renameFile(id: number, name: string) {
    renamingItem = null;
    try {
      await renameNoteFile(id, name);
      if (selectedFile?.id === id) selectedFile = { ...selectedFile, name };
      await loadTree();
    } catch (e) { notify(String(e)); }
  }

  // --- Drag & Drop ---
  // Conserve en local : le DnD des fichiers gere a la fois le reordonnancement ET le
  // deplacement entre dossiers (meme geste), et cohabite avec le DnD des dossiers via
  // le meme etat `dragItem`. Non migrable proprement vers use:reorderable (reorder pur).
  type DragItem = { type: "folder"; id: number } | { type: "file"; id: number; folderId: number | null };

  let dragItem: DragItem | null = $state(null);
  let folderDropTarget: { index: number; pos: "before" | "after" } | null = $state(null);
  let fileDropTarget: { id: number; pos: "before" | "after" } | null = $state(null);
  let folderMoveTarget: number | null = $state(null);
  let rootDropActive = $state(false);

  function onFolderDragStart(e: DragEvent, folder: NoteFolder, _index: number) {
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

    if (sourceFile.folderId !== contextFolderId) {
      moveNoteFile(sourceFile.id, contextFolderId).then(() => loadTree()).catch(() => loadTree());
      cleanDragState();
      return;
    }

    const folderFiles = filesInFolder(contextFolderId);
    const items = [...folderFiles];
    const srcIdx = items.findIndex(f => f.id === sourceFile.id);
    let tgtIdx = items.findIndex(f => f.id === fileDropTarget!.id);
    if (srcIdx === -1 || tgtIdx === -1) { cleanDragState(); return; }

    const [moved] = items.splice(srcIdx, 1);
    if (srcIdx < tgtIdx) tgtIdx--;
    if (fileDropTarget!.pos === "after") tgtIdx++;
    items.splice(tgtIdx, 0, moved);

    tree.files = [
      ...tree.files.filter(f => f.folder_id !== contextFolderId),
      ...items,
    ];
    reorderNoteFiles(items.map(f => f.id)).catch(() => loadTree());
    cleanDragState();
  }

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

  /// Echap quitte le mode lecture quand AUCUNE note n'est ouverte. Le cas ou une note est
  /// ouverte appartient a NoteEditor : lui seul sait conserver la position de lecture, et les
  /// deux branches sont exclusives ({#if selectedFile}), donc aucun risque de double bascule.
  function onEchap(e: KeyboardEvent) {
    if (e.key !== "Escape" || selectedFile || !$readingMode) return;
    if (!notesEl || !(e.target instanceof Node) || !notesEl.contains(e.target)) return;
    readingMode.set(false);
  }

  function cleanDragState() {
    dragItem = null;
    folderDropTarget = null;
    fileDropTarget = null;
    folderMoveTarget = null;
    rootDropActive = false;
  }
</script>

<svelte:window onkeydown={onEchap} />

<div class="notes" bind:this={notesEl}>
  <!-- Mode lecture : l'arborescence est MASQUEE, pas demontee — elle garde ainsi sa position
       de defilement et un renommage en cours. Le retour se fait par le bouton reste dans
       l'en-tete de la note (ou par celui de l'etat vide, plus bas). -->
  <div class="tree-panel" class:replie={$readingMode}>
    <div class="tree-header">
      <h3>{$trad("notes.title")}</h3>
      <button class="sm-btn" onclick={addFolder} title={$trad("notes.newFolder")}>📁+</button>
      <button class="sm-btn" onclick={() => addFile(null)} title={$trad("notes.newFile")}>📄+</button>
    </div>

    {#each tree.folders as folder, fi}
      <div
        class="folder"
        class:dragging={dragItem?.type === "folder" && dragItem.id === folder.id}
        class:drag-over-top={folderDropTarget?.index === fi && folderDropTarget?.pos === "before"}
        class:drag-over-bottom={folderDropTarget?.index === fi && folderDropTarget?.pos === "after"}
        role="listitem"
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
          role="group"
          ondragover={(e) => onFolderHeaderDragOver(e, folder.id)}
          ondragleave={onFolderHeaderDragLeave}
          ondrop={(e) => onFolderHeaderDrop(e, folder.id)}
        >
          {#if renamingItem?.type === "folder" && renamingItem.id === folder.id}
            <InlineEdit
              value={folder.name}
              onCommit={(v) => renameFolder(folder.id, v)}
              onCancel={() => (renamingItem = null)}
            />
          {:else}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span ondblclick={() => (renamingItem = { type: "folder", id: folder.id })}>📁 {folder.name}</span>
          {/if}
          <button class="sm-btn" onclick={() => addFile(folder.id)}>+</button>
          <button class="sm-btn danger" onclick={() => removeFolder(folder.id)}>×</button>
        </div>
        {#each filesInFolder(folder.id) as file}
          <div
            class="file-item"
            class:active={selectedFile?.id === file.id}
            class:dragging={dragItem?.type === "file" && dragItem.id === file.id}
            class:drag-over-top={fileDropTarget?.id === file.id && fileDropTarget?.pos === "before"}
            class:drag-over-bottom={fileDropTarget?.id === file.id && fileDropTarget?.pos === "after"}
            role="button"
            tabindex="0"
            draggable="true"
            ondragstart={(e) => onFileDragStart(e, file)}
            ondragover={(e) => onFileDragOver(e, file)}
            ondragleave={onFileDragLeave}
            ondrop={(e) => onFileDrop(e, folder.id)}
            ondragend={onDragEnd}
            onclick={() => openFile(file.id)}
            ondblclick={() => (renamingItem = { type: "file", id: file.id })}
            onkeydown={(e) => { if (e.key === "Enter") openFile(file.id); }}
          >
            {#if renamingItem?.type === "file" && renamingItem.id === file.id}
              <InlineEdit
                value={file.name}
                onCommit={(v) => renameFile(file.id, v)}
                onCancel={() => (renamingItem = null)}
              />
            {:else}
              <span class="file-label">📄 {file.name}</span>
            {/if}
            <button class="del-btn" type="button" onclick={(e: MouseEvent) => { e.stopPropagation(); removeFile(file.id); }}>×</button>
          </div>
        {/each}
      </div>
    {/each}

    <div
      class="root-files"
      class:root-drop-active={rootDropActive}
      role="list"
      ondragover={onRootDragOver}
      ondragleave={onRootDragLeave}
      ondrop={onRootDrop}
    >
      {#each filesInFolder(null) as file}
        <div
          class="file-item"
          class:active={selectedFile?.id === file.id}
          class:dragging={dragItem?.type === "file" && dragItem.id === file.id}
          class:drag-over-top={fileDropTarget?.id === file.id && fileDropTarget?.pos === "before"}
          class:drag-over-bottom={fileDropTarget?.id === file.id && fileDropTarget?.pos === "after"}
          role="button"
          tabindex="0"
          draggable="true"
          ondragstart={(e) => onFileDragStart(e, file)}
          ondragover={(e) => onFileDragOver(e, file)}
          ondragleave={onFileDragLeave}
          ondrop={(e) => onFileDrop(e, null)}
          ondragend={onDragEnd}
          onclick={() => openFile(file.id)}
          ondblclick={() => (renamingItem = { type: "file", id: file.id })}
          onkeydown={(e) => { if (e.key === "Enter") openFile(file.id); }}
        >
          {#if renamingItem?.type === "file" && renamingItem.id === file.id}
            <InlineEdit
              value={file.name}
              onCommit={(v) => renameFile(file.id, v)}
              onCancel={() => (renamingItem = null)}
            />
          {:else}
            <span class="file-label">📄 {file.name}</span>
          {/if}
          <button class="del-btn" type="button" onclick={(e: MouseEvent) => { e.stopPropagation(); removeFile(file.id); }}>×</button>
        </div>
      {/each}
    </div>
  </div>

  {#if selectedFile}
    <NoteEditor file={selectedFile} onRename={(name) => renameFile(selectedFile!.id, name)} />
  {:else}
    <div class="editor-panel empty-editor">
      <p>{$readingMode ? $trad("notes.readingNoFile") : $trad("notes.selectFile")}</p>
      {#if $readingMode}
        <ReadingToggle />
      {/if}
    </div>
  {/if}
</div>

<style>
  .notes { display: flex; gap: 1rem; height: 100%; min-height: 400px; background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }
  .tree-panel { width: 200px; min-width: 200px; overflow-y: auto; border-right: 1px solid var(--border-color); padding-right: 0.75rem; }
  .tree-panel.replie { display: none; }
  .tree-header { display: flex; align-items: center; gap: 0.25rem; margin-bottom: 0.5rem; }
  .tree-header h3 { flex: 1; margin: 0; font-size: 0.95rem; }
  .sm-btn { background: none; border: none; cursor: pointer; font-size: 0.8rem; padding: 0.1rem 0.3rem; color: var(--text-secondary); }
  .sm-btn:hover { color: var(--text-primary); }
  .sm-btn.danger:hover { color: var(--error); }

  .folder { margin-bottom: 0.5rem; border-top: 2px solid transparent; border-bottom: 2px solid transparent; transition: border-color 0.1s; }
  .folder.dragging { opacity: 0.4; }
  .folder.drag-over-top { border-top-color: var(--accent); }
  .folder.drag-over-bottom { border-bottom-color: var(--accent); }
  .folder-header { display: flex; align-items: center; gap: 0.25rem; font-size: 0.85rem; font-weight: 600; padding: 0.2rem 0; border-radius: 4px; transition: outline 0.1s; }
  .folder-header span { flex: 1; }
  .folder-header.drag-target-folder { outline: 2px solid var(--accent); outline-offset: -2px; }

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
  .file-label { flex: 1; cursor: default; }
  .del-btn { margin-left: auto; background: none; border: none; cursor: pointer; color: var(--error); opacity: 0; font-size: 0.9rem; }
  .file-item:hover .del-btn { opacity: 1; }

  .root-files { min-height: 1rem; border-radius: 4px; transition: outline 0.1s; }
  .root-files.root-drop-active { outline: 2px dashed var(--accent); outline-offset: -2px; }

  .editor-panel { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .empty-editor {
    display: flex; flex-direction: column; gap: 0.75rem;
    align-items: center; justify-content: center; color: var(--text-muted);
  }
  .empty-editor p { margin: 0; }
</style>
