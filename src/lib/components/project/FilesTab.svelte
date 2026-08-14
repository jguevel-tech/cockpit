<script lang="ts">
  import { onMount, tick } from "svelte";
  import { highlightCode } from "../../shiki";
  // themeBase : Shiki n a que deux jeux (github-dark / github-light), pas une palette par theme.
  import { themeBase } from "../../stores/appearance";
  import { projects } from "../../stores/projects";
  import { notify } from "../../stores/toast";
  import { listProjectDir, readProjectFile, writeProjectFile, gotoDefinition, searchProject, setClipboard, createProjectFile, createProjectDir, renameProjectEntry, trashProjectEntry } from "../../api/workspace";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import CodeEditor from "../ui/CodeEditor.svelte";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import type { DefLocation, SearchResults, SearchNameHit, SearchContentHit } from "../../types";

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
  let fileSize = $state(0);
  let loadingFile = $state(false);
  let wrapLines = $state(false);
  let codeWrapEl: HTMLDivElement | undefined = $state();

  // Nombre de lignes affiche dans l'en-tete : un \n final ne compte pas une ligne de plus
  const lineCount = $derived.by(() => {
    if (!fileRaw) return 0;
    const n = fileRaw.split("\n").length;
    return fileRaw.endsWith("\n") ? n - 1 : n;
  });

  async function copyRelPath(rel: string) {
    try {
      await setClipboard(rel);
      notify("Chemin copié", "success");
    } catch (e) { notify(String(e)); }
  }

  function copyPath() {
    return copyRelPath(selectedPath);
  }

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
    fileSize = 0;
    try {
      const f = await readProjectFile(project.path, relPath);
      fileSize = f.size;
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

  // --- Gestion de fichiers : clic droit sur l'arbre (creer, renommer, corbeille) ---
  let treeMenu: { x: number; y: number; node: TreeNode | null } | null = $state(null); // null = racine
  let renamingPath: string | null = $state(null);
  let creating: { parentRel: string; kind: "file" | "dir" } | null = $state(null);

  function openTreeMenu(e: MouseEvent, node: TreeNode | null) {
    e.preventDefault();
    e.stopPropagation();
    treeMenu = { x: e.clientX, y: e.clientY, node };
  }

  function parentOf(rel: string): string {
    const i = rel.lastIndexOf("/");
    return i === -1 ? "" : rel.slice(0, i);
  }

  function findNode(nodes: TreeNode[], rel: string): TreeNode | null {
    for (const n of nodes) {
      if (n.rel_path === rel) return n;
      if (n.is_dir && n.children && rel.startsWith(n.rel_path + "/")) {
        const found = findNode(n.children, rel);
        if (found) return found;
      }
    }
    return null;
  }

  async function reloadDir(parentRel: string) {
    if (!project?.path) return;
    if (parentRel === "") { await loadRoot(); return; }
    const node = findNode(tree, parentRel);
    if (!node) { await loadRoot(); return; }
    try {
      node.children = (await listProjectDir(project.path, parentRel)).map(toNode);
      node.expanded = true;
    } catch (e) { notify(String(e)); }
  }

  async function startCreate(parentRel: string, kind: "file" | "dir") {
    if (parentRel !== "") {
      const node = findNode(tree, parentRel);
      if (node && !node.expanded) await toggleDir(node);
    }
    creating = { parentRel, kind };
  }

  async function commitCreate(name: string) {
    const c = creating;
    creating = null;
    if (!c || !name.trim() || !project?.path) return;
    try {
      const rel = c.kind === "file"
        ? await createProjectFile(project.path, c.parentRel, name.trim())
        : await createProjectDir(project.path, c.parentRel, name.trim());
      await reloadDir(c.parentRel);
      if (c.kind === "file") await openFileByPath(rel);
    } catch (e) { notify(String(e)); }
  }

  async function commitRenameEntry(node: TreeNode, name: string) {
    renamingPath = null;
    if (!project?.path || !name.trim() || name.trim() === node.name) return;
    try {
      const oldRel = node.rel_path;
      const newRel = await renameProjectEntry(project.path, oldRel, name.trim());
      const openInside = selectedPath.startsWith(oldRel + "/");
      const wasOpen = selectedPath === oldRel;
      await reloadDir(parentOf(oldRel));
      // Le fichier affiche suit son propre renommage, ou celui d'un dossier parent
      if (wasOpen) await openFileByPath(newRel);
      else if (openInside) await openFileByPath(newRel + "/" + selectedPath.slice(oldRel.length + 1));
    } catch (e) { notify(String(e)); }
  }

  async function deleteEntry(node: TreeNode) {
    if (!project?.path) return;
    if (!confirm(`Mettre « ${node.name} » à la corbeille ?`)) return;
    try {
      await trashProjectEntry(project.path, node.rel_path);
      if (selectedPath === node.rel_path || selectedPath.startsWith(node.rel_path + "/")) {
        selectedPath = "";
        fileHtml = "";
        fileRaw = "";
        fileNotice = "";
      }
      await reloadDir(parentOf(node.rel_path));
    } catch (e) { notify(String(e)); }
  }

  // --- Recherche globale dans le projet (Ctrl+Maj+F) ---
  let globalQuery = $state("");
  let globalResults: SearchResults | null = $state(null);
  let globalSearching = $state(false);
  let globalError = $state("");
  let globalInputEl: HTMLInputElement | undefined = $state();
  // Numero de requete : seule la DERNIERE reponse a le droit d'ecrire l'etat
  // (deux recherches en vol peuvent revenir dans le desordre).
  let searchSeq = 0;
  let searchDebounce: ReturnType<typeof setTimeout> | undefined;

  function onGlobalInput() {
    clearTimeout(searchDebounce);
    if (globalQuery.trim().length < 2) {
      searchSeq++;
      globalResults = null;
      globalError = "";
      globalSearching = false;
      return;
    }
    searchDebounce = setTimeout(runGlobalSearch, 300);
  }

  async function runGlobalSearch() {
    if (!project?.path) return;
    const q = globalQuery.trim();
    if (q.length < 2) return;
    const seq = ++searchSeq;
    globalSearching = true;
    globalError = "";
    try {
      const res = await searchProject(project.path, q);
      if (seq !== searchSeq) return;
      globalResults = res;
    } catch (e) {
      if (seq !== searchSeq) return;
      globalError = String(e);
      globalResults = null;
    } finally {
      if (seq === searchSeq) globalSearching = false;
    }
  }

  function clearGlobalSearch() {
    clearTimeout(searchDebounce);
    searchSeq++;
    globalQuery = "";
    globalResults = null;
    globalError = "";
    globalSearching = false;
  }

  // Occurrences de contenu groupees par fichier, comme dans un IDE
  const contentGroups = $derived.by(() => {
    const map = new Map<string, SearchContentHit[]>();
    for (const c of globalResults?.contents ?? []) {
      if (!map.has(c.rel_path)) map.set(c.rel_path, []);
      map.get(c.rel_path)!.push(c);
    }
    return [...map.entries()];
  });

  async function openNameHit(hit: SearchNameHit) {
    if (hit.is_dir) await revealDir(hit.rel_path);
    else await openFileByPath(hit.rel_path);
  }

  /** Deplie l'arbre jusqu'au dossier clique dans les resultats, puis revient a l'arbre. */
  async function revealDir(relPath: string) {
    clearGlobalSearch();
    let nodes = tree;
    let acc = "";
    for (const seg of relPath.split("/")) {
      acc = acc ? `${acc}/${seg}` : seg;
      const node = nodes.find((n) => n.rel_path === acc);
      if (!node || !node.is_dir) return;
      if (!node.expanded) await toggleDir(node);
      nodes = node.children ?? [];
    }
  }

  // --- Recherche dans le fichier ouvert (Ctrl+F) ---
  let findOpen = $state(false);
  let findQuery = $state("");
  let findCase = $state(false);
  let findIdx = $state(0);
  let findInputEl: HTMLInputElement | undefined = $state();

  interface FindMatch { line: number; start: number; end: number }
  const FIND_MAX = 2000;

  const findMatches: FindMatch[] = $derived.by(() => {
    if (!findOpen || !findQuery || !fileRaw || editing) return [];
    const out: FindMatch[] = [];
    const needle = findCase ? findQuery : findQuery.toLowerCase();
    const lines = fileRaw.split("\n");
    for (let i = 0; i < lines.length; i++) {
      const hay = findCase ? lines[i] : lines[i].toLowerCase();
      let idx = 0;
      while ((idx = hay.indexOf(needle, idx)) !== -1) {
        out.push({ line: i, start: idx, end: idx + needle.length });
        idx += needle.length;
        if (out.length >= FIND_MAX) return out;
      }
    }
    return out;
  });

  function openFind() {
    if (editing || !fileRaw) return;
    findOpen = true;
    tick().then(() => {
      findInputEl?.focus();
      findInputEl?.select();
    });
  }

  function closeFind() {
    findOpen = false;
  }

  function findNext(dir: 1 | -1) {
    const n = findMatches.length;
    if (!n) return;
    findIdx = ((findIdx + dir) % n + n) % n;
  }

  // Le rendu Shiki est du HTML statique : on surligne en enveloppant les segments
  // de texte matches dans des <mark> (comme la recherche native d'un navigateur).
  // Les <mark> ne changent ni le texte ni ses metriques (fond seul), donc le
  // goto-definition (offsets cumules sur les noeuds texte) reste exact.
  function clearFindMarks() {
    if (!codeWrapEl) return;
    const dirtyLines = new Set<Element>();
    codeWrapEl.querySelectorAll("mark.find-match").forEach((m) => {
      const parent = m.parentNode;
      if (!parent) return;
      const line = (m as HTMLElement).closest(".line");
      if (line) dirtyLines.add(line);
      while (m.firstChild) parent.insertBefore(m.firstChild, m);
      parent.removeChild(m);
    });
    dirtyLines.forEach((l) => l.normalize());
  }

  function applyFindMarks() {
    if (!codeWrapEl) return;
    const lineEls = codeWrapEl.querySelectorAll("code .line");
    findMatches.forEach((m, i) => {
      const lineEl = lineEls[m.line];
      if (lineEl) markRange(lineEl, m.start, m.end, i === findIdx);
    });
  }

  function markRange(lineEl: Element, start: number, end: number, current: boolean) {
    // Un match peut chevaucher plusieurs tokens Shiki : on decoupe par noeud texte.
    const walker = document.createTreeWalker(lineEl, NodeFilter.SHOW_TEXT);
    const targets: { node: Text; s: number; e: number }[] = [];
    let offset = 0;
    let n: Node | null;
    while ((n = walker.nextNode())) {
      const t = n as Text;
      const s = Math.max(start - offset, 0);
      const e = Math.min(end - offset, t.length);
      if (s < e) targets.push({ node: t, s, e });
      offset += t.length;
      if (offset >= end) break;
    }
    for (const t of targets) {
      const range = document.createRange();
      range.setStart(t.node, t.s);
      range.setEnd(t.node, t.e);
      const mark = document.createElement("mark");
      mark.className = current ? "find-match current" : "find-match";
      try { range.surroundContents(mark); } catch { /* impossible : range dans un seul noeud */ }
    }
  }

  // findIdx doit rester valide quand la liste retrecit (frappe, changement de fichier)
  $effect(() => {
    if (findIdx >= findMatches.length) findIdx = 0;
  });

  // (Re)pose les surlignages apres chaque rendu : le {@html} remplace le DOM du code.
  $effect(() => {
    void fileHtml; void findMatches; void findIdx; void findOpen;
    tick().then(() => {
      clearFindMarks();
      if (!findOpen || findMatches.length === 0) return;
      applyFindMarks();
      codeWrapEl?.querySelector("mark.find-match.current")?.scrollIntoView({ block: "center" });
    });
  });

  function onShortcuts(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.shiftKey && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      globalInputEl?.focus();
      globalInputEl?.select();
    } else if (mod && !e.shiftKey && (e.key === "f" || e.key === "F")) {
      if (!editing && fileRaw) {
        e.preventDefault();
        openFind();
      }
    } else if (e.key === "Escape" && findOpen) {
      closeFind();
    }
  }
</script>

<svelte:window
  onkeydown={(e) => { onKeyState(e); onShortcuts(e); }}
  onkeyup={onKeyState}
  onblur={() => (ctrlHeld = false)}
/>

{#snippet createRow(depth: number)}
  <div class="tree-row create-row" style="padding-left: {depth * 0.9 + 0.4}rem">
    <span class="tree-icon">{creating?.kind === "dir" ? "▸" : "·"}</span>
    <InlineEdit value="" onCommit={commitCreate} onCancel={() => (creating = null)} />
  </div>
{/snippet}

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
      oncontextmenu={(e) => openTreeMenu(e, node)}
    >
      <span class="tree-icon">
        {#if node.is_dir}{node.loading ? "…" : node.expanded ? "▾" : "▸"}{:else}·{/if}
      </span>
      {#if renamingPath === node.rel_path}
        <InlineEdit
          value={node.name}
          onCommit={(v) => commitRenameEntry(node, v)}
          onCancel={() => (renamingPath = null)}
        />
      {:else}
        <span class="tree-name" class:dir={node.is_dir}>{node.name}</span>
      {/if}
    </div>
    {#if node.is_dir && creating?.parentRel === node.rel_path}
      {@render createRow(depth + 1)}
    {/if}
    {#if node.is_dir && node.expanded && node.children}
      {@render nodeList(node.children, depth + 1)}
    {/if}
  {/each}
{/snippet}

<div class="files-tab">
  <div class="files-tree">
    <div class="tree-header">
      <span>Fichiers</span>
      <span class="tree-header-actions">
        <button class="tree-refresh" onclick={() => startCreate("", "file")} title="Nouveau fichier à la racine">+·</button>
        <button class="tree-refresh" onclick={() => startCreate("", "dir")} title="Nouveau dossier à la racine">+▸</button>
        <button class="tree-refresh" onclick={loadRoot} title="Rafraîchir">↻</button>
      </span>
    </div>
    <div class="global-search">
      <input
        bind:this={globalInputEl}
        bind:value={globalQuery}
        oninput={onGlobalInput}
        placeholder="Rechercher dans le projet…"
        title="Noms de dossiers, de fichiers et contenu (Ctrl+Maj+F)"
        onkeydown={(e) => { if (e.key === "Escape") { clearGlobalSearch(); e.currentTarget.blur(); } }}
      />
      {#if globalQuery}
        <button class="search-clear" onclick={clearGlobalSearch} title="Effacer la recherche">×</button>
      {/if}
    </div>
    {#if globalQuery.trim().length >= 2}
      <div class="search-results">
        {#if globalSearching}<p class="search-note">Recherche…</p>{/if}
        {#if globalError}<p class="tree-error">{globalError}</p>{/if}
        {#if globalResults}
          {#if globalResults.names.length === 0 && globalResults.contents.length === 0}
            <p class="search-note">Aucun résultat</p>
          {/if}
          {#if globalResults.names.length > 0}
            <div class="search-section">Noms · {globalResults.names.length}</div>
            {#each globalResults.names as hit (hit.rel_path)}
              <div
                class="tree-row"
                role="button"
                tabindex="0"
                onclick={() => openNameHit(hit)}
                onkeydown={(e) => e.key === "Enter" && openNameHit(hit)}
              >
                <span class="tree-icon">{hit.is_dir ? "▸" : "·"}</span>
                <span class="tree-name" class:dir={hit.is_dir} title={hit.rel_path}>{hit.rel_path}</span>
              </div>
            {/each}
          {/if}
          {#if globalResults.contents.length > 0}
            <div class="search-section">Contenu · {globalResults.contents.length}{globalResults.truncated ? "+" : ""}</div>
            {#each contentGroups as [path, hits] (path)}
              <div class="search-file" title={path}>{path} <span class="search-count">{hits.length}</span></div>
              {#each hits as h (h.line)}
                <div
                  class="tree-row search-hit"
                  role="button"
                  tabindex="0"
                  onclick={() => openAt(h.rel_path, h.line)}
                  onkeydown={(e) => e.key === "Enter" && openAt(h.rel_path, h.line)}
                >
                  <span class="hit-line">{h.line + 1}</span>
                  <span class="hit-preview">{h.preview}</span>
                </div>
              {/each}
            {/each}
          {/if}
          {#if globalResults.truncated}
            <p class="search-note">Résultats limités — précise la recherche.</p>
          {/if}
        {/if}
      </div>
    {:else if treeError}
      <p class="tree-error">{treeError}</p>
    {:else}
      {#if creating?.parentRel === ""}
        {@render createRow(0)}
      {/if}
      {@render nodeList(tree, 0)}
    {/if}
  </div>

  <div class="files-viewer" class:editing>
    {#if selectedPath}
      <div class="viewer-header">
        <div class="viewer-header-row">
          <code>{selectedPath}</code>
          <button class="icon-mini" onclick={copyPath} title="Copier le chemin">⧉</button>
          {#if dirty}<span class="dirty-dot" title="Modifications non sauvegardées">●</span>{/if}
          <span class="viewer-actions">
            {#if defBusy}<span class="def-busy">définition…</span>{/if}
            {#if fileRaw && !editing}
              <span class="file-stats">{lineCount} lignes · {formatSize(fileSize)}</span>
              <button class="icon-mini" class:active={wrapLines} onclick={() => (wrapLines = !wrapLines)} title="Retour à la ligne automatique">⏎</button>
              <button class="icon-mini" onclick={openFind} title="Rechercher dans le fichier (Ctrl+F)">🔍</button>
            {/if}
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
        {#if findOpen && !editing}
          <div class="find-bar">
            <input
              bind:this={findInputEl}
              bind:value={findQuery}
              placeholder="Rechercher dans le fichier…"
              class:no-match={findQuery.length > 0 && findMatches.length === 0}
              onkeydown={(e) => {
                if (e.key === "Enter") { e.preventDefault(); findNext(e.shiftKey ? -1 : 1); }
                else if (e.key === "Escape") closeFind();
              }}
            />
            <button class="icon-mini" class:active={findCase} onclick={() => (findCase = !findCase)} title="Respecter la casse">Aa</button>
            <span class="find-count">{findMatches.length ? `${findIdx + 1}/${findMatches.length}${findMatches.length >= 2000 ? "+" : ""}` : "0/0"}</span>
            <button class="icon-mini" onclick={() => findNext(-1)} title="Occurrence précédente (Maj+Entrée)">↑</button>
            <button class="icon-mini" onclick={() => findNext(1)} title="Occurrence suivante (Entrée)">↓</button>
            <button class="icon-mini" onclick={closeFind} title="Fermer (Échap)">×</button>
          </div>
        {/if}
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
          class:wrap={wrapLines}
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

{#if treeMenu}
  {@const n = treeMenu.node}
  <ContextMenu
    x={treeMenu.x}
    y={treeMenu.y}
    items={[
      ...(n === null || n.is_dir
        ? [
            { label: "Nouveau fichier", action: () => startCreate(n?.rel_path ?? "", "file") },
            { label: "Nouveau dossier", action: () => startCreate(n?.rel_path ?? "", "dir") },
          ]
        : []),
      ...(n !== null
        ? [
            { label: "Renommer", action: () => (renamingPath = n.rel_path) },
            { label: "Copier le chemin", action: () => copyRelPath(n.rel_path) },
            { label: "Mettre à la corbeille", danger: true, action: () => deleteEntry(n) },
          ]
        : []),
    ]}
    onClose={() => (treeMenu = null)}
  />
{/if}

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
  .tree-header-actions { display: flex; gap: 0.15rem; }
  .tree-refresh {
    background: none; border: none; cursor: pointer; color: var(--text-muted);
    font-size: 0.9rem; padding: 0 0.2rem;
  }
  .tree-refresh:hover { color: var(--accent); }
  .create-row { background: var(--bg-tertiary); }
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

  /* Recherche globale (panneau de gauche) */
  .global-search {
    display: flex; align-items: center; gap: 0.3rem;
    padding: 0 0.5rem 0.4rem; border-bottom: 1px solid var(--border-color);
    margin-bottom: 0.3rem;
  }
  .global-search input {
    flex: 1; min-width: 0; font-size: 0.78rem; padding: 0.25rem 0.45rem;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary);
  }
  .search-clear {
    background: none; border: none; color: var(--text-muted); cursor: pointer;
    font-size: 0.95rem; padding: 0 0.2rem; line-height: 1;
  }
  .search-clear:hover { color: var(--text-primary); }
  .search-section {
    padding: 0.4rem 0.6rem 0.15rem; font-size: 0.7rem; font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted);
  }
  .search-file {
    padding: 0.3rem 0.6rem 0.05rem; font-size: 0.75rem; font-weight: 600;
    color: var(--text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .search-count { color: var(--text-muted); font-weight: 400; }
  .search-hit { gap: 0.4rem; }
  .hit-line { color: var(--text-muted); font-size: 0.7rem; min-width: 2.5ch; text-align: right; flex-shrink: 0; }
  .hit-preview {
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    font-family: var(--font-mono, monospace); font-size: 0.74rem;
  }
  .search-note { padding: 0.4rem 0.6rem; font-size: 0.75rem; color: var(--text-muted); }

  .files-viewer {
    flex: 1; min-width: 0; overflow: auto;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-secondary);
  }
  .viewer-header {
    padding: 0.4rem 0.8rem; border-bottom: 1px solid var(--border-color);
    font-size: 0.8rem; position: sticky; top: 0; background: var(--bg-secondary); z-index: 1;
  }
  .viewer-header-row { display: flex; align-items: center; gap: 0.5rem; }
  .viewer-actions { margin-left: auto; display: flex; gap: 0.4rem; align-items: center; }
  .file-stats { color: var(--text-muted); font-size: 0.72rem; white-space: nowrap; }
  .icon-mini {
    background: none; border: 1px solid transparent; border-radius: 4px;
    color: var(--text-muted); cursor: pointer; font-size: 0.8rem;
    padding: 0.05rem 0.3rem; line-height: 1.3;
  }
  .icon-mini:hover { color: var(--text-primary); border-color: var(--border-color); }
  .icon-mini.active { color: var(--accent); border-color: var(--accent); }

  /* Barre de recherche dans le fichier (Ctrl+F) — deuxieme ligne de l'en-tete sticky */
  .find-bar { display: flex; align-items: center; gap: 0.35rem; padding-top: 0.4rem; }
  .find-bar input {
    flex: 0 1 16rem; min-width: 6rem; font-size: 0.78rem; padding: 0.2rem 0.45rem;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary);
  }
  .find-bar input.no-match { border-color: var(--error); }
  .find-count { color: var(--text-muted); font-size: 0.72rem; min-width: 3.5ch; text-align: center; }
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
    margin: 0; padding: 0.8rem 1rem 0.8rem 0.5rem; overflow-x: auto;
    background: transparent !important;
  }
  .code-wrap :global(code) { font-family: var(--font-mono, monospace); line-height: 1.5; counter-reset: line; }
  /* Numeros de ligne : compteur CSS sur chaque .line de Shiki. Un ::before ne cree
     PAS de noeud texte : le goto-definition et le surlignage de recherche (offsets
     cumules sur les noeuds texte) restent exacts, et la copie ne l'embarque pas. */
  .code-wrap :global(.line)::before {
    counter-increment: line;
    content: counter(line);
    display: inline-block;
    width: 3.2em;
    padding-right: 1em;
    text-align: right;
    color: var(--text-muted);
    opacity: 0.55;
    user-select: none;
  }
  /* Retour a la ligne automatique (bouton ⏎) : pour le Markdown, les logs... */
  .code-wrap.wrap :global(pre) { white-space: pre-wrap; word-break: break-word; }
  /* Surlignage des occurrences (Ctrl+F) : fond seul, aucune metrique modifiee */
  .code-wrap :global(mark.find-match) {
    background: color-mix(in srgb, var(--warning) 38%, transparent);
    color: inherit; padding: 0; border-radius: 2px;
  }
  .code-wrap :global(mark.find-match.current) {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
    outline: 1px solid var(--accent);
  }
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
