<script lang="ts">
  import { untrack } from "svelte";
  import { marked } from "marked";
  import TurndownService from "turndown";
  import { saveNoteFile } from "../../api/storage";
  import { notify } from "../../stores/toast";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import type { NoteFile } from "../../types";
  import { trad } from "../../i18n";

  let {
    file,
    onRename,
  }: {
    file: NoteFile;
    onRename?: (name: string) => void;
  } = $props();

  const turndown = new TurndownService({ headingStyle: "atx", codeBlockStyle: "fenced" });

  let markdownContent = $state("");
  let editorEl: HTMLDivElement | undefined = $state(undefined);
  let renaming = $state(false);

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let currentId: number | null = null;
  let dirty = false;

  // Change de fichier : on flush la sauvegarde en attente de l'ancien avant de charger le nouveau.
  $effect(() => {
    const f = file; // dependance : reagit au changement de fichier
    untrack(() => switchTo(f));
  });

  function switchTo(f: NoteFile) {
    if (f.id === currentId) return;
    flush();
    currentId = f.id;
    markdownContent = f.content || "";
    dirty = false;
    requestAnimationFrame(() => {
      if (editorEl) editorEl.innerHTML = marked.parse(markdownContent) as string;
    });
  }

  async function flush() {
    if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
    if (!dirty || currentId === null) return;
    const id = currentId;
    const content = markdownContent;
    dirty = false;
    try { await saveNoteFile(id, content); } catch (e) { notify(String(e)); }
  }

  function onEditorInput() {
    if (!editorEl) return;
    markdownContent = turndown.turndown(editorEl.innerHTML);
    dirty = true;
    scheduleSave();
  }

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => { flush(); }, 1000);
  }

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
</script>

<div class="editor-panel">
  <div class="editor-header">
    {#if renaming}
      <InlineEdit
        value={file.name}
        onCommit={(v) => { renaming = false; onRename?.(v); }}
        onCancel={() => (renaming = false)}
      />
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="file-title" ondblclick={() => (renaming = true)}>{file.name}</span>
    {/if}
    <div class="toolbar">
      <button class="tb" onclick={() => format("bold")} title={$trad("note.bold")}><b>G</b></button>
      <button class="tb" onclick={() => format("italic")} title={$trad("note.italic")}><i>I</i></button>
      <button class="tb" onclick={() => format("strikeThrough")} title={$trad("note.strike")}><s>S</s></button>
      <span class="tb-sep"></span>
      <button class="tb" onclick={() => insertHeading(1)} title={$trad("note.h1")}>H1</button>
      <button class="tb" onclick={() => insertHeading(2)} title={$trad("note.h2")}>H2</button>
      <button class="tb" onclick={() => insertHeading(3)} title={$trad("note.h3")}>H3</button>
      <span class="tb-sep"></span>
      <button class="tb" onclick={() => format("insertUnorderedList")} title={$trad("note.list")}>•</button>
      <button class="tb" onclick={() => format("insertOrderedList")} title={$trad("note.orderedList")}>1.</button>
      <button class="tb" onclick={() => format("formatBlock", "blockquote")} title={$trad("note.quote")}>❝</button>
      <span class="tb-sep"></span>
      <button class="tb" onclick={() => format("formatBlock", "pre")} title={$trad("note.codeBlock")}>&lt;/&gt;</button>
      <button class="tb" onclick={() => { const url = prompt("URL :"); if (url) format("createLink", url); }} title={$trad("note.link")}>🔗</button>
    </div>
  </div>

  <div
    bind:this={editorEl}
    class="editor"
    contenteditable="true"
    oninput={onEditorInput}
  ></div>
</div>

<style>
  .editor-panel { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .editor-header { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem; flex-wrap: wrap; }
  .file-title { font-weight: 600; font-size: 0.9rem; }

  .toolbar { display: flex; align-items: center; gap: 0.15rem; margin-left: auto; flex-wrap: wrap; }
  .tb {
    background: var(--bg-tertiary); border: 1px solid var(--border-color); color: var(--text-secondary);
    padding: 0.2rem 0.45rem; border-radius: 4px; cursor: pointer; font-size: 0.75rem;
    line-height: 1; min-width: 24px; text-align: center;
  }
  .tb:hover { background: var(--accent); color: white; border-color: var(--accent); }
  .tb-sep { width: 1px; height: 16px; background: var(--border-color); margin: 0 0.2rem; }

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
</style>
