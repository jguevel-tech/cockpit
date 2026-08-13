<script lang="ts">
  import { onMount } from "svelte";
  import { highlightCode } from "../../shiki";
  // themeBase : Shiki n a que deux jeux (github-dark / github-light), pas une palette par theme.
  import { themeBase } from "../../stores/appearance";
  import { projects } from "../../stores/projects";
  import { notify } from "../../stores/toast";
  import { listProjectDir, readProjectFile, writeProjectFile, gotoDefinition } from "../../api/workspace";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import CodeEditor from "../ui/CodeEditor.svelte";
  import type { DefLocation } from "../../types";

  let { name }: { name: string } = $props();

  interface TreeNode {
    name: string;
    rel_path: string;
    is_dir: boolean;
    expanded: boolean;
    loading: boolean;
    children: TreeNode[] | null;
  }

  let tree: TreeNode[] = $state([]);
  let treeError = $state("");
  let selectedPath = $state("");
  let fileHtml = $state("");
  let fileRaw = $state("");
  let fileNotice = $state("");
  let fileTruncated = $state(false);
  let loadingFile = $state(false);
  let codeWrapEl: HTMLDivElement | undefined = $state();

  // Edition
  let editing = $state(false);
  let draft = $state("");
  let saving = $state(false);
  const dirty = $derived(editing && draft !== fileRaw);

  // Aller a la definition
  let defBusy = $state(false);
  let ctrlHeld = $state(false);
  let defMenu: { x: number; y: number; hits: DefLocation[] } | null = $state(null);

  const project = $derived($projects.find((p) => p.name === name));

  const LANG_BY_EXT: Record<string, string> = {
    rs: "rust", ts: "typescript", js: "javascript", mjs: "javascript", cjs: "javascript",
    svelte: "svelte", vue: "vue", py: "python", php: "php", rb: "ruby", go: "go",
    json: "json", yaml: "yaml", yml: "yaml", toml: "toml", xml: "xml",
    md: "markdown", html: "html", css: "css", scss: "scss", less: "less",
    sh: "bash", bash: "bash", zsh: "bash", sql: "sql", java: "java", kt: "kotlin",
    c: "c", h: "c", cpp: "cpp", hpp: "cpp", cs: "csharp", swift: "swift",
    dockerfile: "dockerfile", conf: "ini", ini: "ini", env: "ini", lock: "text",
    txt: "text", log: "text", csv: "text", tsx: "tsx", jsx: "jsx",
  };

  function langFor(path: string): string {
    const base = path.split("/").pop() ?? "";
    if (/^dockerfile/i.test(base)) return "dockerfile";
    if (/^makefile$/i.test(base)) return "makefile";
    const ext = base.includes(".") ? base.split(".").pop()!.toLowerCase() : "";
    return LANG_BY_EXT[ext] ?? "text";
  }

  onMount(loadRoot);

  async function loadRoot() {
    treeError = "";
    if (!project?.path) { treeError = "Chemin du projet inconnu"; return; }
    try {
      tree = (await listProjectDir(project.path, "")).map(toNode);
    } catch (e) { treeError = String(e); }
  }

  function toNode(e: { name: string; rel_path: string; is_dir: boolean }): TreeNode {
    return { ...e, expanded: false, loading: false, children: null };
  }

  async function toggleDir(node: TreeNode) {
    if (!node.is_dir || !project?.path) return;
    if (node.expanded) { node.expanded = false; return; }
    if (node.children === null) {
      node.loading = true;
      try {
        node.children = (await listProjectDir(project.path, node.rel_path)).map(toNode);
      } catch { node.children = []; }
      finally { node.loading = false; }
    }
    node.expanded = true;
  }

  function openFile(node: TreeNode) {
    openFileByPath(node.rel_path);
  }

  async function openFileByPath(relPath: string) {
    if (!project?.path) return;
    if (dirty && !confirm("Modifications non sauvegardées — les abandonner ?")) return;
    editing = false;
    selectedPath = relPath;
    loadingFile = true;
    fileHtml = "";
    fileRaw = "";
    fileNotice = "";
    fileTruncated = false;
    try {
      const f = await readProjectFile(project.path, relPath);
      if (f.binary) {
        fileNotice = `Fichier binaire (${formatSize(f.size)})`;
        return;
      }
      fileTruncated = f.truncated;
      if (f.truncated) fileNotice = `Fichier tronqué à 2 Mo (taille réelle : ${formatSize(f.size)})`;
      fileRaw = f.content;
      fileHtml = await highlightCode(f.content, langFor(relPath), $themeBase === "dark");
    } catch (e) { fileNotice = String(e); }
    finally { loadingFile = false; }
  }

  function formatSize(bytes: number): string {
    if (bytes > 1024 * 1024) return (bytes / 1024 / 1024).toFixed(1) + " Mo";
    if (bytes > 1024) return (bytes / 1024).toFixed(1) + " Ko";
    return bytes + " o";
  }

  // --- Edition ---
  function startEdit() {
    if (fileTruncated) { notify("Fichier tronqué à 2 Mo : édition désactivée"); return; }
    if (!fileRaw && fileNotice) return;
    draft = fileRaw;
    editing = true;
  }

  function cancelEdit() {
    if (dirty && !confirm("Modifications non sauvegardées — les abandonner ?")) return;
    editing = false;
  }

  async function save() {
    if (!project?.path || !editing || saving) return;
    saving = true;
    try {
      await writeProjectFile(project.path, selectedPath, draft);
      fileRaw = draft;
      fileHtml = await highlightCode(draft, langFor(selectedPath), $themeBase === "dark");
      notify("Fichier sauvegardé", "success");
    } catch (e) { notify(String(e)); }
    finally { saving = false; }
  }

  // --- Aller a la definition (Ctrl+clic) ---

  // Mots-cles courants (php/js/ts/rust/py) : pas des cibles de definition
  const KEYWORDS = new Set([
    "if", "else", "elseif", "for", "foreach", "while", "do", "switch", "case",
    "break", "continue", "return", "function", "fn", "def", "class", "interface",
    "trait", "enum", "struct", "type", "extends", "implements", "new", "use",
    "as", "try", "catch", "finally", "throw", "echo", "print", "self", "static",
    "public", "private", "protected", "const", "let", "var", "pub", "mut",
    "async", "await", "match", "impl", "mod", "crate", "super", "this",
    "true", "false", "null", "undefined", "None", "True", "False", "import",
    "export", "from", "default", "void", "string", "int", "float", "bool",
    "array", "mixed", "callable", "instanceof", "and", "or", "not", "in", "of",
    "is", "pass", "require", "include", "namespace", "readonly", "global",
  ]);

  interface WordHit {
    line: number;
    character: number;
    symbol: string;
    node: Text;
    startInNode: number;
    endInNode: number;
  }

  /** Symbole EXACT sous le curseur (mot dans le token Shiki), avec sa position LSP. */
  function wordAtPoint(e: MouseEvent): WordHit | null {
    const doc = document as Document & {
      caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null;
      caretRangeFromPoint?: (x: number, y: number) => Range | null;
    };
    let node: Node | null = null;
    let offset = 0;
    const cp = doc.caretPositionFromPoint?.(e.clientX, e.clientY);
    if (cp) { node = cp.offsetNode; offset = cp.offset; }
    else {
      const range = doc.caretRangeFromPoint?.(e.clientX, e.clientY);
      if (range) { node = range.startContainer; offset = range.startOffset; }
    }
    if (!node || node.nodeType !== Node.TEXT_NODE) return null;

    const lineEl = node.parentElement?.closest(".line");
    const codeEl = lineEl?.closest("code");
    if (!lineEl || !codeEl) return null;
    const lines = Array.from(codeEl.querySelectorAll(".line"));
    const line = lines.indexOf(lineEl);
    if (line < 0) return null;

    // Bornes du mot DANS le noeud de texte clique (un identifiant = un token)
    const isWord = (c: string) => /[A-Za-z0-9_]/.test(c);
    const nodeText = node.textContent ?? "";
    if (offset >= nodeText.length || !isWord(nodeText[offset])) return null;
    let startInNode = offset;
    let endInNode = offset;
    while (startInNode > 0 && isWord(nodeText[startInNode - 1])) startInNode--;
    while (endInNode < nodeText.length && isWord(nodeText[endInNode])) endInNode++;
    const symbol = nodeText.slice(startInNode, endInNode);
    if (!symbol || /^\d+$/.test(symbol) || KEYWORDS.has(symbol)) return null;

    // Colonne LSP = texte des noeuds precedents dans la ligne + debut du mot
    let character = startInNode;
    const walker = document.createTreeWalker(lineEl, NodeFilter.SHOW_TEXT);
    let n: Node | null;
    while ((n = walker.nextNode())) {
      if (n === node) break;
      character += n.textContent?.length ?? 0;
    }

    return { line, character, symbol, node: node as Text, startInNode, endInNode };
  }

  // Soulignement precis du symbole survole (Ctrl enfonce), style IDE
  let hoverLink: { left: number; top: number; width: number; height: number } | null = $state(null);

  function onCodeMove(e: MouseEvent) {
    if (!ctrlHeld || !codeWrapEl) { hoverLink = null; return; }
    const w = wordAtPoint(e);
    if (!w) { hoverLink = null; return; }
    const range = document.createRange();
    range.setStart(w.node, w.startInNode);
    range.setEnd(w.node, w.endInNode);
    const r = range.getBoundingClientRect();
    const host = codeWrapEl.getBoundingClientRect();
    hoverLink = { left: r.left - host.left, top: r.top - host.top, width: r.width, height: r.height };
  }

  async function onCodeClick(e: MouseEvent) {
    if (!(e.ctrlKey || e.metaKey) || !project?.path || defBusy) return;
    const pos = wordAtPoint(e);
    if (!pos) return;
    e.preventDefault();
    defBusy = true;
    try {
      const res = await gotoDefinition(
        project.path, langFor(selectedPath), selectedPath, fileRaw,
        pos.line, pos.character, pos.symbol,
      );
      if (res.hits.length === 0) {
        notify(`Définition introuvable pour « ${pos.symbol} »`, "info");
      } else if (res.hits.length === 1) {
        await openAt(res.hits[0].rel_path, res.hits[0].line);
      } else {
        defMenu = { x: e.clientX, y: e.clientY, hits: res.hits };
      }
    } catch (e2) { notify(String(e2)); }
    finally { defBusy = false; }
  }

  async function openAt(relPath: string, line: number) {
    if (relPath !== selectedPath) await openFileByPath(relPath);
    requestAnimationFrame(() => scrollToLine(line));
  }

  function scrollToLine(line: number) {
    const lines = codeWrapEl?.querySelectorAll("code .line");
    const el = lines?.[line] as HTMLElement | undefined;
    if (!el) return;
    el.scrollIntoView({ block: "center" });
    el.classList.add("jump-target");
    setTimeout(() => el.classList.remove("jump-target"), 1800);
  }

  function onKeyState(e: KeyboardEvent) {
    ctrlHeld = e.ctrlKey || e.metaKey;
    if (!ctrlHeld) hoverLink = null;
  }
</script>

<svelte:window onkeydown={onKeyState} onkeyup={onKeyState} onblur={() => (ctrlHeld = false)} />

{#snippet nodeList(nodes: TreeNode[], depth: number)}
  {#each nodes as node (node.rel_path)}
    <div
      class="tree-row"
      class:selected={selectedPath === node.rel_path && !node.is_dir}
      style="padding-left: {depth * 0.9 + 0.4}rem"
      role="button"
      tabindex="0"
      onclick={() => (node.is_dir ? toggleDir(node) : openFile(node))}
      onkeydown={(e) => e.key === "Enter" && (node.is_dir ? toggleDir(node) : openFile(node))}
    >
      <span class="tree-icon">
        {#if node.is_dir}{node.loading ? "…" : node.expanded ? "▾" : "▸"}{:else}·{/if}
      </span>
      <span class="tree-name" class:dir={node.is_dir}>{node.name}</span>
    </div>
    {#if node.is_dir && node.expanded && node.children}
      {@render nodeList(node.children, depth + 1)}
    {/if}
  {/each}
{/snippet}

<div class="files-tab">
  <div class="files-tree">
    <div class="tree-header">
      <span>Fichiers</span>
      <button class="tree-refresh" onclick={loadRoot} title="Rafraîchir">↻</button>
    </div>
    {#if treeError}
      <p class="tree-error">{treeError}</p>
    {:else}
      {@render nodeList(tree, 0)}
    {/if}
  </div>

  <div class="files-viewer" class:editing>
    {#if selectedPath}
      <div class="viewer-header">
        <code>{selectedPath}</code>{#if dirty}<span class="dirty-dot" title="Modifications non sauvegardées">●</span>{/if}
        <span class="viewer-actions">
          {#if defBusy}<span class="def-busy">définition…</span>{/if}
          {#if editing}
            <button class="btn small primary" onclick={save} disabled={saving || !dirty} title="Ctrl+S">
              {saving ? "Sauvegarde…" : "Sauvegarder"}
            </button>
            <button class="btn small" onclick={cancelEdit}>Lecture</button>
          {:else if fileRaw}
            <button class="btn small" onclick={startEdit} title="Modifier le fichier">✎ Modifier</button>
          {/if}
        </span>
      </div>
      {#if fileNotice}<p class="viewer-notice">{fileNotice}</p>{/if}
      {#if loadingFile}
        <p class="viewer-notice">Chargement…</p>
      {:else if editing}
        <div class="editor-host">
          <CodeEditor bind:value={draft} lang={langFor(selectedPath)} dark={$themeBase === "dark"} onSave={save} />
        </div>
      {:else if fileHtml}
        <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
        <div
          class="code-wrap"
          class:linkable={!!hoverLink}
          bind:this={codeWrapEl}
          onclick={onCodeClick}
          onmousemove={onCodeMove}
          onmouseleave={() => (hoverLink = null)}
        >
          {@html fileHtml}
          {#if hoverLink}
            <div
              class="symbol-link"
              style="left: {hoverLink.left}px; top: {hoverLink.top}px; width: {hoverLink.width}px; height: {hoverLink.height}px"
            ></div>
          {/if}
        </div>
      {/if}
    {:else}
      <div class="viewer-empty">Sélectionne un fichier pour voir son contenu</div>
    {/if}
  </div>
</div>

{#if defMenu}
  <ContextMenu
    x={defMenu.x}
    y={defMenu.y}
    items={defMenu.hits.map((h) => ({
      label: `${h.rel_path}:${h.line + 1}`,
      action: () => openAt(h.rel_path, h.line),
    }))}
    onClose={() => (defMenu = null)}
  />
{/if}

<style>
  .files-tab { display: flex; gap: 1rem; height: 100%; min-height: 0; }
  .files-tree {
    width: 280px; flex-shrink: 0; overflow-y: auto;
    border: 1px solid var(--border-color); border-radius: 6px;
    padding: 0.4rem 0; background: var(--bg-secondary);
  }
  .tree-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0 0.6rem 0.4rem; font-size: 0.8rem; font-weight: 600;
    color: var(--text-muted); border-bottom: 1px solid var(--border-color);
    margin-bottom: 0.3rem;
  }
  .tree-refresh {
    background: none; border: none; cursor: pointer; color: var(--text-muted);
    font-size: 0.9rem; padding: 0 0.2rem;
  }
  .tree-refresh:hover { color: var(--accent); }
  .tree-row {
    display: flex; align-items: center; gap: 0.35rem;
    padding: 0.12rem 0.4rem; cursor: pointer; font-size: 0.82rem;
    color: var(--text-secondary); white-space: nowrap; overflow: hidden;
  }
  .tree-row:hover { background: var(--bg-tertiary); }
  .tree-row.selected { background: var(--bg-tertiary); color: var(--accent); }
  .tree-icon { width: 0.9rem; text-align: center; color: var(--text-muted); flex-shrink: 0; }
  .tree-name.dir { font-weight: 600; color: var(--text-primary); }
  .tree-error { padding: 0.6rem; color: var(--error, #e5484d); font-size: 0.85rem; }

  .files-viewer {
    flex: 1; min-width: 0; overflow: auto;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-secondary);
  }
  .viewer-header {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.4rem 0.8rem; border-bottom: 1px solid var(--border-color);
    font-size: 0.8rem; position: sticky; top: 0; background: var(--bg-secondary); z-index: 1;
  }
  .viewer-actions { margin-left: auto; display: flex; gap: 0.4rem; align-items: center; }
  .dirty-dot { color: var(--warning); font-size: 0.7rem; }
  .def-busy { color: var(--text-muted); font-size: 0.75rem; font-style: italic; }
  /* En mode edition, le conteneur ne scrolle plus : c'est l'editeur qui scrolle */
  .files-viewer.editing { display: flex; flex-direction: column; overflow: hidden; }
  .editor-host { flex: 1; min-height: 0; }
  .viewer-notice { padding: 0.6rem 0.8rem; color: var(--text-muted); font-size: 0.85rem; }
  .viewer-empty {
    display: flex; align-items: center; justify-content: center; height: 100%;
    color: var(--text-muted); font-size: 0.85rem;
  }
  .code-wrap { font-size: 0.82rem; }
  .code-wrap :global(pre) {
    margin: 0; padding: 0.8rem 1rem; overflow-x: auto;
    background: transparent !important;
  }
  .code-wrap :global(code) { font-family: var(--font-mono, monospace); line-height: 1.5; }
  .code-wrap { position: relative; }
  .code-wrap.linkable { cursor: pointer; }
  /* Soulignement du SEUL symbole sous le curseur (Ctrl enfonce), style IDE */
  .symbol-link {
    position: absolute; pointer-events: none;
    background: var(--accent-soft);
    border-bottom: 1px solid var(--accent);
    border-radius: 2px;
  }
  /* Ligne cible apres un saut de definition */
  .code-wrap :global(.line.jump-target) {
    background: var(--accent-soft);
    transition: background 0.4s ease;
  }
</style>
