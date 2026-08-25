<script lang="ts">
  import { onMount } from "svelte";
  import { projects } from "../../stores/projects";
  import {
    gitStatus, gitDiffFile, gitStage, gitUnstage, gitStageAll, gitUnstageAll,
    gitCommit, gitPush, gitPull, gitLog, gitCommitDiff,
    gitBranches, gitCheckoutBranch, gitCreateBranch, gitDeleteBranch,
    gitWorktrees, gitWorktreeAdd, gitWorktreeRemove,
  } from "../../api/workspace";
  import type { GitStatus, GitStatusEntry, FileDiff, BranchInfo, CommitInfo, Worktree } from "../../types";
  import { pendingTerminalCommand, activeTab } from "../../stores/ui";
  import { notify } from "../../stores/toast";
  import { trad } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";
  import { demanderConfirmation } from "../../stores/confirm";

  let { name }: { name: string } = $props();

  let status: GitStatus | null = $state(null);
  let statusError = $state("");
  let loadingStatus = $state(false);
  let busy = $state(""); // libelle de l'operation en cours (desactive les boutons)

  let selectedPath = $state("");
  let selectedStaged = $state(false);
  let diff: FileDiff | null = $state(null);
  let diffError = $state("");
  let loadingDiff = $state(false);

  let commitMsg = $state("");

  // Historique
  let view: "changes" | "history" | "worktrees" = $state("changes");
  let commits: CommitInfo[] = $state([]);
  let commitsError = $state("");
  let loadingCommits = $state(false);
  let selectedCommit: CommitInfo | null = $state(null);
  let commitFiles: FileDiff[] = $state([]);
  let loadingCommitDiff = $state(false);

  // Menu branches
  let branchMenuOpen = $state(false);
  let branches: BranchInfo[] = $state([]);
  let newBranchName = $state("");
  let creatingBranch = $state(false);

  const project = $derived($projects.find((p) => p.name === name));

  const stagedFiles: GitStatusEntry[] = $derived.by(() => status?.files.filter((f) => f.staged) ?? []);
  const unstagedFiles: GitStatusEntry[] = $derived.by(() => status?.files.filter((f) => f.unstaged) ?? []);

  const STATUS_LABELS: Record<string, { label: string; cls: string }> = {
    M: { label: "M", cls: "mod" },
    A: { label: "A", cls: "add" },
    D: { label: "D", cls: "del" },
    R: { label: "R", cls: "mod" },
    C: { label: "C", cls: "mod" },
    U: { label: "U", cls: "del" },
    "??": { label: "?", cls: "new" },
  };
  function badge(s: string) {
    return STATUS_LABELS[s] ?? { label: s, cls: "mod" };
  }

  onMount(refresh);

  async function refresh() {
    if (!project?.path) { statusError = $trad("project.unknownPath"); return; }
    loadingStatus = true;
    statusError = "";
    try {
      status = await gitStatus(project.path);
      if (selectedPath && !status.files.some((f) => f.path === selectedPath)) {
        selectedPath = "";
        diff = null;
      } else if (selectedPath) {
        await openDiff(selectedPath, selectedStaged);
      }
    } catch (e) {
      signalerErreur("git.refresh", String(e)); statusError = String(e); }
    finally { loadingStatus = false; }
  }

  async function openDiff(path: string, staged: boolean) {
    if (!project?.path) return;
    selectedPath = path;
    selectedStaged = staged;
    loadingDiff = true;
    diff = null;
    diffError = "";
    try {
      const untracked = status?.files.find((f) => f.path === path)?.untracked ?? false;
      diff = await gitDiffFile(project.path, path, untracked);
    } catch (e) {
      signalerErreur("git.openDiff", String(e)); diffError = String(e); }
    finally { loadingDiff = false; }
  }

  // Wrapper : execute une operation git, gere busy + erreurs + refresh
  async function op(label: string, fn: () => Promise<unknown>, after?: () => void) {
    if (!project?.path || busy) return;
    busy = label;
    try {
      await fn();
      await refresh();
      after?.();
    } catch (e) {
      notify(String(e));
    } finally {
      busy = "";
    }
  }

  const stage = (f: GitStatusEntry) => op("stage", () => gitStage(project!.path, f.path));
  const unstage = (f: GitStatusEntry) => op("unstage", () => gitUnstage(project!.path, f.path));
  const stageAll = () => op("stageAll", () => gitStageAll(project!.path));
  const unstageAll = () => op("unstageAll", () => gitUnstageAll(project!.path));

  function doCommit() {
    const msg = commitMsg.trim();
    if (!msg) return;
    op("commit", () => gitCommit(project!.path, msg), () => { commitMsg = ""; });
  }

  async function doPush() {
    if (!project?.path || busy) return;
    busy = "push";
    try {
      await gitPush(project.path, !status?.has_upstream);
      await refresh();
    } catch (e) {
      signalerErreur("git.doPush", String(e));
      // Branche sans upstream : proposer --set-upstream
      const msg = String(e);
      const sansUpstream = /upstream/i.test(msg)
        && (await demanderConfirmation({ message: $trad("git.noUpstreamConfirm"), action: $trad("common.confirm"), danger: false }));
      if (sansUpstream) {
        try { await gitPush(project.path, true); await refresh(); } catch (e2) { notify(String(e2)); }
      } else {
        notify(String(e));
      }
    } finally { busy = ""; }
  }

  function doPull() {
    op("pull", async () => {
      const out = await gitPull(project!.path);
      notify(out.split("\n")[0] || $trad("git.upToDate"), "success");
      if (view === "history") await loadHistory();
    });
  }

  async function showView(v: "changes" | "history" | "worktrees") {
    view = v;
    if (v === "history" && commits.length === 0) await loadHistory();
    if (v === "worktrees") await loadWorktrees();
  }

  // --- Worktrees ---
  // Un worktree est un second dossier de travail sur le meme depot, sur une autre branche :
  // c'est ce qui permet de faire tourner plusieurs agents en parallele sans qu'un `checkout`
  // change le code sous les pieds des autres.
  let worktrees: Worktree[] = $state([]);
  let worktreesError = $state("");
  let loadingWorktrees = $state(false);
  let nouvelleBranche = $state("");

  // La branche saisie existe-t-elle deja ? Ca decide entre « ajouter » et « creer et ajouter »,
  // et le libelle du bouton le DIT avant le clic — on ne fait pas deviner.
  let brancheExiste = $derived(
    branches.some((b) => b.name === nouvelleBranche.trim()),
  );

  async function loadWorktrees() {
    if (!project?.path) return;
    loadingWorktrees = true;
    worktreesError = "";
    try {
      worktrees = await gitWorktrees(project.path);
      // La liste des branches sert a savoir s'il faut en creer une : on la charge ici aussi,
      // sinon le libelle du bouton serait faux tant que le menu des branches n'a pas ete ouvert.
      if (branches.length === 0) branches = await gitBranches(project.path);
    } catch (e) {
      signalerErreur("git.loadWorktrees", String(e));
      worktreesError = String(e);
    } finally {
      loadingWorktrees = false;
    }
  }

  async function ajouterWorktree() {
    const branche = nouvelleBranche.trim();
    if (!branche) { notify($trad("git.worktreeBranchNeeded")); return; }
    if (!project?.path) { notify($trad("git.noProjectPath")); return; }
    busy = "worktree";
    try {
      const chemin = await gitWorktreeAdd(project.path, branche, !brancheExiste);
      nouvelleBranche = "";
      notify($trad("git.worktreeAdded", { chemin }), "success");
      await loadWorktrees();
      branches = await gitBranches(project.path);
    } catch (e) {
      notify(String(e));
    } finally {
      busy = "";
    }
  }

  async function retirerWorktree(w: Worktree) {
    if (!project?.path) { notify($trad("git.noProjectPath")); return; }
    const question = $trad("git.worktreeRemoveConfirm", { chemin: w.chemin });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    busy = "worktree";
    try {
      await gitWorktreeRemove(project.path, w.chemin, false);
      await loadWorktrees();
    } catch (e) {
      // Git refuse un worktree qui porte des modifications non validees. On le dit, et on
      // propose d'insister — plutot que de forcer d'office, ce qui perdrait du travail.
      const insister = $trad("git.worktreeRemoveForce", { erreur: String(e) });
      if (await demanderConfirmation({ message: insister, action: $trad("common.delete") })) {
        try {
          await gitWorktreeRemove(project.path, w.chemin, true);
          await loadWorktrees();
        } catch (e2) { notify(String(e2)); }
      }
    } finally {
      busy = "";
    }
  }

  /// Ouvre un terminal DANS le worktree. C'est l'onglet Terminal qui cree la session (lui seul
  /// mesure son conteneur, donc la taille du PTY) : on depose la demande et on y navigue.
  function terminalDansWorktree(w: Worktree) {
    pendingTerminalCommand.set({ project: name, command: "", dossier: w.chemin });
    activeTab.set("terminal");
  }

  async function loadHistory() {
    if (!project?.path) return;
    loadingCommits = true;
    commitsError = "";
    try { commits = await gitLog(project.path, 100); } catch (e) {
      signalerErreur("git.loadHistory", String(e)); commitsError = String(e); }
    finally { loadingCommits = false; }
  }

  async function openCommit(c: CommitInfo) {
    if (!project?.path) return;
    selectedCommit = c;
    loadingCommitDiff = true;
    commitFiles = [];
    try { commitFiles = await gitCommitDiff(project.path, c.full_hash); } catch (e) { notify(String(e)); }
    finally { loadingCommitDiff = false; }
  }

  function relativeTime(epoch: number): string {
    const diff = Math.floor(Date.now() / 1000) - epoch;
    if (diff < 3600) return $trad("time.minutesAgo", { n: Math.max(1, Math.floor(diff / 60)) });
    if (diff < 86400) return $trad("time.hoursAgo", { n: Math.floor(diff / 3600) });
    if (diff < 86400 * 30) return $trad("time.daysAgoShort", { n: Math.floor(diff / 86400) });
    return new Date(epoch * 1000).toLocaleDateString();
  }

  async function toggleBranchMenu() {
    branchMenuOpen = !branchMenuOpen;
    if (branchMenuOpen && project?.path) {
      try { branches = await gitBranches(project.path); } catch (e) {
      signalerErreur("git.toggleBranchMenu", String(e)); branches = []; }
    }
  }

  function switchBranch(b: BranchInfo) {
    if (b.current) { branchMenuOpen = false; return; }
    op("checkout", () => gitCheckoutBranch(project!.path, b.name), () => { branchMenuOpen = false; });
  }

  function createBranch() {
    const n = newBranchName.trim();
    if (!n) return;
    op("createBranch", () => gitCreateBranch(project!.path, n), () => {
      newBranchName = "";
      creatingBranch = false;
      branchMenuOpen = false;
    });
  }

  async function deleteBranch(b: BranchInfo, e: MouseEvent) {
    e.stopPropagation();
    if (!project?.path || b.current) return;
    const question = $trad("git.deleteBranchConfirm", { name: b.name });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    try {
      await gitDeleteBranch(project.path, b.name, false);
    } catch (err) {
      signalerErreur("git.deleteBranch", String(err));
      // Non mergee : proposer le force delete
      const forcer = $trad("git.forceDeleteConfirm", { name: b.name });
      if (await demanderConfirmation({ message: forcer, action: $trad("common.delete") })) {
        try { await gitDeleteBranch(project.path, b.name, true); } catch (e2) { notify(String(e2)); return; }
      } else return;
    }
    branches = await gitBranches(project.path);
  }
</script>

<div class="git-tab">
  <div class="git-panel">
    <!-- Barre branche + actions -->
    <div class="git-topbar">
      <div class="branch-select">
        <button class="branch-btn" onclick={toggleBranchMenu} disabled={!status?.is_repo}>
          ⎇ {status?.branch || "—"} ▾
        </button>
        {#if branchMenuOpen}
          <div class="branch-menu">
            {#each branches as b (b.name)}
              <div class="branch-row" class:current={b.current}>
                <button class="branch-name" onclick={() => switchBranch(b)}>
                  {b.current ? "● " : ""}{b.name}
                </button>
                {#if !b.current}
                  <button class="branch-del" title={$trad("git.deleteBranch")} onclick={(e) => deleteBranch(b, e)}>🗑</button>
                {/if}
              </div>
            {/each}
            {#if creatingBranch}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="branch-new-input"
                bind:value={newBranchName}
                placeholder={$trad("git.newBranchPlaceholder")}
                spellcheck="false"
                autofocus
                onkeydown={(e) => { if (e.key === "Enter") createBranch(); if (e.key === "Escape") creatingBranch = false; }}
              />
            {:else}
              <button class="branch-new" onclick={() => (creatingBranch = true)}>{$trad("git.newBranch")}</button>
            {/if}
          </div>
        {/if}
      </div>

      {#if status?.is_repo}
        <div class="git-totals">
          <span class="stat-add">+{status.total_additions}</span>
          <span class="stat-del">−{status.total_deletions}</span>
        </div>
        <button class="pull-btn" onclick={doPull} disabled={!!busy} title={$trad("git.pullHint")}>
          {busy === "pull" ? "Pull…" : "⬇ Pull"}
          {#if status.behind}<span class="ahead">{status.behind}</span>{/if}
        </button>
        <button class="push-btn" onclick={doPush} disabled={!!busy} title={status.has_upstream ? "git push" : "git push --set-upstream"}>
          {busy === "push" ? "Push…" : "⬆ Push"}
          {#if status.ahead}<span class="ahead">{status.ahead}</span>{/if}
        </button>
        <button class="icon-btn" onclick={refresh} disabled={loadingStatus} title={$trad("common.refresh")}>↻</button>
      {/if}
    </div>

    {#if statusError}
      <p class="git-msg error">{statusError}</p>
    {:else if status && !status.is_repo}
      <p class="git-msg">{$trad("git.notARepo")}</p>
    {:else if status}
      <div class="git-views">
        <button class:active={view === "changes"} onclick={() => showView("changes")}>{$trad("git.changes")}</button>
        <button class:active={view === "history"} onclick={() => showView("history")}>{$trad("git.history")}</button>
        <button class:active={view === "worktrees"} onclick={() => showView("worktrees")}>{$trad("git.worktrees")}</button>
      </div>

      {#if view === "worktrees"}
        <div class="worktree-panel">
          <p class="git-msg hint">{$trad("git.worktreesHint")}</p>
          <div class="worktree-add">
            <input
              class="input"
              bind:value={nouvelleBranche}
              placeholder={$trad("git.worktreeBranchPlaceholder")}
              spellcheck="false"
              onkeydown={(e) => { if (e.key === "Enter") ajouterWorktree(); }}
            />
            <button class="btn" onclick={ajouterWorktree} disabled={busy === "worktree" || !nouvelleBranche.trim()}>
              {brancheExiste ? $trad("git.worktreeAdd") : $trad("git.worktreeCreateAndAdd")}
            </button>
          </div>

          {#if loadingWorktrees}
            <p class="git-msg">{$trad("common.loading")}</p>
          {:else if worktreesError}
            <p class="git-msg error">{worktreesError}</p>
          {:else}
            {#each worktrees as w (w.chemin)}
              <div class="worktree-row">
                <div class="worktree-id">
                  <span class="worktree-branch">
                    {w.branche ?? $trad("git.worktreeDetached", { tete: w.tete })}
                    {#if w.principal}<span class="badge">{$trad("git.worktreeMain")}</span>{/if}
                    {#if w.verrouille}<span class="badge">{$trad("git.worktreeLocked")}</span>{/if}
                    {#if w.elagable}<span class="badge">{$trad("git.worktreePrunable")}</span>{/if}
                  </span>
                  <span class="worktree-path" title={w.chemin}>{w.chemin}</span>
                </div>
                <button class="icon-btn" title={$trad("git.worktreeTerminal")} onclick={() => terminalDansWorktree(w)}>▶</button>
                {#if !w.principal}
                  <button class="icon-btn" title={$trad("git.worktreeRemove")} onclick={() => retirerWorktree(w)} disabled={busy === "worktree"}>🗑</button>
                {/if}
              </div>
            {/each}
          {/if}
        </div>
      {/if}

      {#if view === "history"}
        <div class="commit-list">
          {#if loadingCommits}
            <p class="git-msg">{$trad("common.loading")}</p>
          {:else if commitsError}
            <p class="git-msg error">{commitsError}</p>
          {:else if commits.length === 0}
            <p class="git-msg">{$trad("git.noCommit")}</p>
          {:else}
            {#each commits as c (c.full_hash)}
              <button class="commit-row" class:selected={selectedCommit?.full_hash === c.full_hash} onclick={() => openCommit(c)}>
                <span class="commit-subject" title={c.subject}>{c.subject}</span>
                <span class="commit-meta">
                  <code class="commit-hash">{c.hash}</code>
                  {c.author} · {relativeTime(c.epoch)}
                  {#if c.refs}<span class="commit-refs" title={c.refs}>{c.refs}</span>{/if}
                </span>
              </button>
            {/each}
          {/if}
        </div>
      {:else}
      {#if status.behind}
        <p class="git-behind">{$trad("git.behind", { n: status.behind })}</p>
      {/if}

      <div class="file-groups">
        <!-- Staged -->
        {#if stagedFiles.length > 0}
          <div class="group-head">
            <span>{$trad("git.staged")} ({stagedFiles.length})</span>
            <button class="link-btn" onclick={unstageAll} disabled={!!busy}>{$trad("git.unstageAll")}</button>
          </div>
          {#each stagedFiles as f (f.path)}
            <div class="git-file staged" class:selected={selectedPath === f.path}>
              <button class="file-btn" onclick={() => openDiff(f.path, true)}>
                <span class="git-badge {badge(f.status).cls}">{badge(f.status).label}</span>
                <span class="git-path" title={f.path}>{f.path}</span>
                <span class="file-stat"><span class="stat-add">+{f.additions}</span> <span class="stat-del">−{f.deletions}</span></span>
              </button>
              <button class="stage-btn" title={$trad("git.unstage")} onclick={() => unstage(f)} disabled={!!busy}>−</button>
            </div>
          {/each}
        {/if}

        <!-- Unstaged -->
        {#if unstagedFiles.length > 0}
          <div class="group-head">
            <span>{$trad("git.changes")} ({unstagedFiles.length})</span>
            <button class="link-btn" onclick={stageAll} disabled={!!busy}>{$trad("git.stageAll")}</button>
          </div>
          {#each unstagedFiles as f (f.path)}
            <div class="git-file" class:selected={selectedPath === f.path}>
              <button class="file-btn" onclick={() => openDiff(f.path, false)}>
                <span class="git-badge {badge(f.status).cls}">{badge(f.status).label}</span>
                <span class="git-path" title={f.path}>{f.path}</span>
                <span class="file-stat"><span class="stat-add">+{f.additions}</span> <span class="stat-del">−{f.deletions}</span></span>
              </button>
              <button class="stage-btn add" title={$trad("git.stage")} onclick={() => stage(f)} disabled={!!busy}>+</button>
            </div>
          {/each}
        {/if}

        {#if status.files.length === 0}
          <p class="git-msg">{$trad("git.noChange")}</p>
        {/if}
      </div>

      <!-- Commit -->
      {#if stagedFiles.length > 0}
        <div class="commit-box">
          <textarea
            bind:value={commitMsg}
            placeholder={$trad("git.commitPlaceholder")}
            rows="2"
            onkeydown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) doCommit(); }}
          ></textarea>
          <button class="commit-btn" onclick={doCommit} disabled={!commitMsg.trim() || !!busy}>
            {busy === "commit" ? "Commit…" : `Commit (${stagedFiles.length})`}
          </button>
        </div>
      {/if}
      {/if}
    {/if}
  </div>

  <div class="git-diff">
    {#if view === "history"}
      {#if selectedCommit}
        <div class="diff-header">
          <code>{selectedCommit.hash}</code>
          <span class="commit-header-subject" title={selectedCommit.subject}>{selectedCommit.subject}</span>
          <span class="diff-stats">{selectedCommit.author} · {relativeTime(selectedCommit.epoch)}</span>
        </div>
        {#if loadingCommitDiff}
          <p class="git-msg">{$trad("common.loading")}</p>
        {:else if commitFiles.length === 0}
          <p class="git-msg">{$trad("git.noDiffInCommit")}</p>
        {:else}
          {#each commitFiles as file (file.path)}
            <div class="commit-file-head">
              <code>{file.path}</code>
              <span class="diff-stats">
                <span class="stat-add">+{file.additions}</span>
                <span class="stat-del">−{file.deletions}</span>
              </span>
            </div>
            <table class="diff-table">
              <tbody>
                {#each file.hunks as hunk}
                  <tr class="hunk-row">
                    <td class="lineno"></td>
                    <td class="lineno"></td>
                    <td class="line-text">{hunk.header}</td>
                  </tr>
                  {#each hunk.lines as line}
                    <tr class="line-{line.kind}">
                      <td class="lineno">{line.old_line ?? ""}</td>
                      <td class="lineno">{line.new_line ?? ""}</td>
                      <td class="line-text"><span class="line-sign">{line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span>{line.text}</td>
                    </tr>
                  {/each}
                {/each}
              </tbody>
            </table>
          {/each}
        {/if}
      {:else}
        <div class="diff-empty">{$trad("git.selectCommit")}</div>
      {/if}
    {:else if selectedPath}
      <div class="diff-header">
        <code>{selectedPath}</code>
        {#if diff}
          <span class="diff-stats">
            <span class="stat-add">+{diff.additions}</span>
            <span class="stat-del">−{diff.deletions}</span>
          </span>
        {/if}
      </div>
      {#if loadingDiff}
        <p class="git-msg">{$trad("common.loading")}</p>
      {:else if diffError}
        <p class="git-msg error">{diffError}</p>
      {:else if diff && diff.hunks.length === 0}
        <p class="git-msg">{$trad("git.noDiffContent")}</p>
      {:else if diff}
        <table class="diff-table">
          <tbody>
            {#each diff.hunks as hunk}
              <tr class="hunk-row">
                <td class="lineno"></td>
                <td class="lineno"></td>
                <td class="line-text">{hunk.header}</td>
              </tr>
              {#each hunk.lines as line}
                <tr class="line-{line.kind}">
                  <td class="lineno">{line.old_line ?? ""}</td>
                  <td class="lineno">{line.new_line ?? ""}</td>
                  <td class="line-text"><span class="line-sign">{line.kind === "add" ? "+" : line.kind === "del" ? "−" : " "}</span>{line.text}</td>
                </tr>
              {/each}
            {/each}
          </tbody>
        </table>
      {/if}
    {:else}
      <div class="diff-empty">{$trad("git.selectFile")}</div>
    {/if}
  </div>
</div>

<style>
  .git-tab { display: flex; gap: 1rem; height: 100%; min-height: 0; }
  .git-panel {
    width: 340px; flex-shrink: 0; display: flex; flex-direction: column; min-height: 0;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-secondary);
  }

  .git-topbar {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem 0.6rem; border-bottom: 1px solid var(--border-color);
  }
  .branch-select { position: relative; }
  .branch-btn {
    background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 4px;
    color: var(--accent); font-family: monospace; font-size: 0.8rem; cursor: pointer;
    padding: 0.25rem 0.5rem; max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .branch-btn:disabled { opacity: 0.5; }
  .branch-menu {
    position: absolute; left: 0; top: calc(100% + 4px); z-index: 20; width: 260px;
    max-height: 320px; overflow-y: auto;
    background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 6px;
    box-shadow: 0 6px 20px rgba(0,0,0,0.3); padding: 0.25rem;
  }
  .branch-row { display: flex; align-items: center; }
  .branch-row .branch-name {
    flex: 1; min-width: 0; text-align: left; background: none; border: none; cursor: pointer;
    color: var(--text-secondary); font-family: monospace; font-size: 0.78rem;
    padding: 0.3rem 0.4rem; border-radius: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .branch-row.current .branch-name { color: var(--accent); font-weight: 600; }
  .branch-row:hover { background: var(--bg-tertiary); }
  .branch-del { background: none; border: none; cursor: pointer; opacity: 0; font-size: 0.75rem; padding: 0 0.3rem; }
  .branch-row:hover .branch-del { opacity: 0.7; }
  .branch-del:hover { opacity: 1; }
  .branch-new, .branch-new-input {
    display: block; width: 100%; margin-top: 0.25rem; padding: 0.3rem 0.4rem;
    font-size: 0.78rem; border-radius: 4px;
  }
  .branch-new { background: none; border: none; color: var(--accent); cursor: pointer; text-align: left; }
  .branch-new:hover { background: var(--bg-tertiary); }
  .branch-new-input {
    border: 1px solid var(--accent); background: var(--bg-primary); color: var(--text-primary);
    font-family: monospace; outline: none;
  }

  .git-totals { font-family: monospace; font-size: 0.8rem; display: flex; gap: 0.4rem; }
  .push-btn {
    margin-left: auto; display: flex; align-items: center; gap: 0.3rem;
    padding: 0.25rem 0.6rem; font-size: 0.78rem; cursor: pointer;
    background: var(--accent); color: white; border: none; border-radius: 4px;
  }
  .push-btn:disabled { opacity: 0.5; }
  .push-btn .ahead {
    background: rgba(255,255,255,0.25); border-radius: 8px; padding: 0 0.35rem; font-size: 0.7rem;
  }
  .icon-btn { background: none; border: none; cursor: pointer; color: var(--text-muted); font-size: 0.9rem; }
  .icon-btn:hover { color: var(--accent); }

  .git-behind { margin: 0; padding: 0.35rem 0.6rem; font-size: 0.75rem; color: var(--warning, #d29922); background: rgba(210,153,34,0.1); }
  .pull-btn {
    margin-left: auto; display: flex; align-items: center; gap: 0.3rem;
    padding: 0.25rem 0.6rem; font-size: 0.78rem; cursor: pointer;
    background: var(--bg-tertiary); color: var(--text-primary);
    border: 1px solid var(--border-color); border-radius: 4px;
  }
  .pull-btn:disabled { opacity: 0.5; }
  .pull-btn .ahead { background: var(--accent-soft); border-radius: 8px; padding: 0 0.35rem; font-size: 0.7rem; }
  /* Le Pull prend le margin-left:auto, le Push reste colle a lui */
  .push-btn { margin-left: 0; }
  .git-views {
    display: flex; gap: 0; border-bottom: 1px solid var(--border-color);
  }
  .git-views button {
    flex: 1; padding: 0.4rem 0; background: none; border: none; cursor: pointer;
    color: var(--text-muted); font-size: 0.78rem; border-bottom: 2px solid transparent;
  }
  .git-views button.active { color: var(--accent); border-bottom-color: var(--accent); }
  .commit-list { flex: 1; overflow-y: auto; min-height: 0; }
  .commit-row {
    display: flex; flex-direction: column; gap: 0.15rem; width: 100%;
    padding: 0.4rem 0.6rem; background: none; border: none; cursor: pointer;
    text-align: left; border-bottom: 1px solid var(--border-color);
  }
  .commit-row:hover, .commit-row.selected { background: var(--bg-tertiary); }
  .commit-subject {
    font-size: 0.8rem; color: var(--text-primary);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .commit-row.selected .commit-subject { color: var(--accent); }
  .commit-meta {
    display: flex; align-items: center; gap: 0.4rem;
    font-size: 0.68rem; color: var(--text-muted);
    overflow: hidden; white-space: nowrap;
  }
  .commit-hash { color: var(--accent); }
  .commit-refs {
    border: 1px solid var(--border-color); border-radius: 8px; padding: 0 0.35rem;
    overflow: hidden; text-overflow: ellipsis; max-width: 12rem;
  }
  .commit-header-subject {
    flex: 1; margin: 0 0.6rem; font-size: 0.8rem;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .commit-file-head {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.35rem 0.8rem; font-size: 0.78rem;
    background: var(--bg-tertiary); border-top: 1px solid var(--border-color);
    border-bottom: 1px solid var(--border-color);
    position: sticky; top: 2.1rem;
  }
  .git-msg { padding: 0.6rem; color: var(--text-muted); font-size: 0.85rem; }
  .git-msg.hint { line-height: 1.45; }

  /* --- Worktrees --- */
  .worktree-panel { overflow-y: auto; display: flex; flex-direction: column; gap: 0.25rem; }
  .worktree-add { display: flex; gap: 0.4rem; padding: 0 0.6rem 0.5rem; }
  .worktree-add .input { flex: 1; min-width: 0; }
  .worktree-row {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.4rem 0.6rem; border-bottom: 1px solid var(--border-color);
  }
  .worktree-id { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 0.15rem; }
  .worktree-branch {
    display: flex; align-items: center; gap: 0.35rem;
    font-size: 0.85rem; color: var(--text-primary);
  }
  /* Le chemin est TOUJOURS montre : rien ne doit apparaitre sur le disque de quelqu'un sans
     qu'il sache ou. Tronque a gauche, la fin d'un chemin etant ce qui distingue. */
  .worktree-path {
    font-size: 0.72rem; color: var(--text-muted); font-family: var(--font-mono);
    direction: rtl; text-align: left; overflow: hidden; text-overflow: ellipsis;
    white-space: nowrap;
  }
  .git-msg.error { color: var(--error, #e5484d); }

  .file-groups { flex: 1; overflow-y: auto; min-height: 0; }
  .group-head {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.35rem 0.6rem; font-size: 0.72rem; text-transform: uppercase; letter-spacing: 0.04em;
    color: var(--text-muted); background: var(--bg-tertiary);
    position: sticky; top: 0;
  }
  .link-btn { background: none; border: none; color: var(--accent); cursor: pointer; font-size: 0.72rem; }
  .link-btn:disabled { opacity: 0.5; }

  .git-file { display: flex; align-items: center; }
  .git-file:hover { background: var(--bg-tertiary); }
  .git-file.selected { background: var(--bg-tertiary); }
  .file-btn {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: 0.5rem;
    padding: 0.28rem 0.6rem; background: none; border: none; cursor: pointer;
    color: var(--text-secondary); text-align: left; font-size: 0.8rem;
  }
  .git-file.selected .file-btn { color: var(--accent); }
  .git-badge {
    width: 1.05rem; text-align: center; border-radius: 3px; flex-shrink: 0;
    font-family: monospace; font-size: 0.72rem; font-weight: 700;
  }
  .git-badge.mod { color: #d29922; }
  .git-badge.add { color: #46a758; }
  .git-badge.del { color: #e5484d; }
  .git-badge.new { color: #6e9fff; }
  .git-path { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; direction: rtl; text-align: left; }
  .file-stat { flex-shrink: 0; font-family: monospace; font-size: 0.68rem; }
  .stage-btn {
    flex-shrink: 0; width: 22px; height: 22px; margin-right: 0.4rem; border-radius: 4px;
    border: 1px solid var(--border-color); background: var(--bg-secondary);
    color: var(--text-secondary); cursor: pointer; font-size: 0.9rem; line-height: 1;
  }
  .stage-btn:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
  .stage-btn.add:hover:not(:disabled) { border-color: #46a758; color: #46a758; }
  .stage-btn:disabled { opacity: 0.4; }

  .commit-box { border-top: 1px solid var(--border-color); padding: 0.5rem; }
  .commit-box textarea {
    width: 100%; padding: 0.4rem 0.5rem; border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary); font-family: inherit; font-size: 0.82rem;
    resize: vertical;
  }
  .commit-btn {
    width: 100%; margin-top: 0.4rem; padding: 0.4rem; font-size: 0.85rem; cursor: pointer;
    background: var(--accent); color: white; border: none; border-radius: 6px;
  }
  .commit-btn:disabled { opacity: 0.5; }

  .stat-add { color: #46a758; }
  .stat-del { color: #e5484d; }

  .git-diff {
    flex: 1; min-width: 0; overflow: auto;
    border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-secondary);
  }
  .diff-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.4rem 0.8rem; border-bottom: 1px solid var(--border-color);
    font-size: 0.8rem; position: sticky; top: 0; background: var(--bg-secondary); z-index: 1;
  }
  .diff-stats { font-family: monospace; font-size: 0.8rem; display: flex; gap: 0.5rem; }
  .diff-empty { display: flex; align-items: center; justify-content: center; height: 100%; color: var(--text-muted); font-size: 0.85rem; }
  .diff-table { width: 100%; border-collapse: collapse; font-family: monospace; font-size: 0.78rem; line-height: 1.45; }
  .lineno {
    width: 3.2em; min-width: 3.2em; text-align: right; padding: 0 0.5em;
    color: var(--text-muted); user-select: none; vertical-align: top;
    border-right: 1px solid var(--border-color);
  }
  .line-text { padding: 0 0.6em; white-space: pre-wrap; word-break: break-all; }
  .line-sign { display: inline-block; width: 1em; user-select: none; }
  .hunk-row .line-text { color: var(--accent); background: var(--bg-tertiary); font-style: italic; }
  .line-add { background: rgba(70, 167, 88, 0.14); }
  .line-add .line-text { color: var(--text-primary); }
  .line-del { background: rgba(229, 72, 77, 0.12); }
  .line-del .line-text { color: var(--text-primary); }
</style>
