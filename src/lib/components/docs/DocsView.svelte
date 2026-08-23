<script lang="ts">
  import { trad, translate } from "../../i18n";
  /**
   * Documentation integree (bouton ⓘ du Header). Principe voulu :
   * TRES PEU de texte, surtout des exemples et des illustrations. Les "captures"
   * sont des maquettes dessinees en HTML/CSS : legeres, nettes a tout zoom, et
   * elles suivent le theme. Chaque bloc = une legende d'une ligne + une maquette.
   */
  type SectionId =
    | "demarrer" | "terminaux" | "fichiers" | "git" | "docker"
    | "taches" | "palette" | "dashboard" | "apparence" | "compte" | "maj" | "raccourcis";

  const MENU: { id: SectionId; labelKey: Parameters<typeof translate>[0]; icon: string }[] = [
    { id: "demarrer", labelKey: "docs.menu.start", icon: "🚀" },
    { id: "dashboard", labelKey: "docs.menu.dashboard", icon: "📊" },
    { id: "terminaux", labelKey: "docs.menu.terminals", icon: ">_" },
    { id: "fichiers", labelKey: "docs.menu.files", icon: "📄" },
    { id: "git", labelKey: "docs.menu.git", icon: "⎇" },
    { id: "docker", labelKey: "docs.menu.docker", icon: "🐳" },
    { id: "taches", labelKey: "docs.menu.tasks", icon: "✓" },
    { id: "palette", labelKey: "docs.menu.palette", icon: "⌘" },
    { id: "apparence", labelKey: "docs.menu.appearance", icon: "🎨" },
    { id: "compte", labelKey: "docs.menu.account", icon: "👤" },
    { id: "maj", labelKey: "docs.menu.updates", icon: "🔔" },
    { id: "raccourcis", labelKey: "docs.menu.shortcuts", icon: "⌨" },
  ];

  let section: SectionId = $state("demarrer");
</script>

<div class="docs">
  <nav>
    <h2>{$trad("docs.title")}</h2>
    {#each MENU as m (m.id)}
      <button class:active={section === m.id} onclick={() => (section = m.id)}>
        <span class="icon">{m.icon}</span>{$trad(m.labelKey)}
      </button>
    {/each}
  </nav>

  <!-- .stack : panneau continu (fond translucide sous wallpaper via components.css) —
       sans lui, titres et legendes flottaient directement sur l'image de fond. -->
  <div class="content stack">
    {#if section === "demarrer"}
      <h3>{$trad("docs.start.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.start.project")}</p>
        <div class="demo">
          <div class="d-side-head">{$trad("sidebar.projects")} <span class="d-btn small">{$trad("sidebar.newProject")}</span> <span class="d-btn small">{$trad("sidebar.newFolder")}</span></div>
          <div class="d-note">{$trad("docs.start.projectDemo")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.start.folders")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-caret">▾</span> <strong>Core</strong> <span class="d-count">12</span> <span class="d-hover">+▸ 🗑</span></div>
          <div class="d-row indent"><span class="d-caret">▾</span> <strong>Back</strong> <span class="d-count">3</span></div>
          <div class="d-row indent2"><span class="d-dot ok"></span> api-gateway <span class="d-muted">running</span></div>
          <div class="d-row indent2"><span class="d-dot"></span> worker <span class="d-muted">stopped</span></div>
          <div class="d-row indent"><span class="d-caret">▸</span> <strong>Front</strong> <span class="d-count">9</span></div>
          <div class="d-note">{$trad("docs.start.foldersDemo")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.start.rename")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-dot ok"></span> <strong>api-gateway</strong> <span class="d-muted">running</span></div>
          <div class="d-menu indent"><div>{$trad("common.rename")}</div></div>
          <div class="d-note">{$trad("docs.start.renameDemo")}</div>
        </div>
      </div>

      <div class="block">
        <p>{$trad("docs.start.bar")}</p>
        <div class="demo">
          <div class="d-tabs">
            <strong>mon-projet</strong> <span class="d-pencil">✎</span>
            <span class="d-tab active">{$trad("tab.workspace")}</span><span class="d-tab">{$trad("tab.docker")}</span><span class="d-tab">{$trad("tab.terminal")}</span><span class="d-tab">{$trad("tab.files")}</span><span class="d-tab">{$trad("tab.git")}</span>
            <span class="d-spring"></span>
            <span class="d-btn small">{$trad("project.runCommand")}</span><span class="d-btn small danger">{$trad("rec.start")}</span>
          </div>
          <div class="d-note">{$trad("docs.start.barMemory")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.start.urls")}</p>
        <div class="demo">
          <div class="d-tabs">
            <span class="d-btn small"><span class="d-dot ok"></span> Préprod</span>
            <span class="d-btn small"><span class="d-dot err"></span> Staging</span>
            <span class="d-note">{$trad("docs.start.urlsDemo")}</span>
          </div>
        </div>
      </div>

    {:else if section === "terminaux"}
      <h3>{$trad("docs.term.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.term.persistent")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-tab active">MON-PROJET - 1 ×</span><span class="d-tab">MON-PROJET - 2 ×</span><span class="d-btn small">+</span><span class="d-btn small">{$trad("term.claudeMenu")}</span><span class="d-btn small">🔍</span></div>
          <div class="d-term">$ npm run dev<br />VITE ready in 320 ms ➜ http://localhost:5173</div>
          <div class="d-note">{$trad("docs.term.persistentDemo")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.term.search")}</p>
        <div class="demo">
          <div class="d-term">…<br />ERROR connection <mark>timeout</mark> after 30s <span class="d-float">(1/4)</span><br />retrying…</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.term.copy")}</p>
        <div class="demo">
          <div class="d-term">$ cat config.yml<br /><mark>database_url: postgres://…</mark> {$trad("docs.term.copyDemo")}<br />$</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.term.drop")}</p>
        <div class="demo">
          <div class="d-term">{@html $trad("docs.term.dropDemo1")}</div>
          <div class="d-note">{$trad("docs.term.dropDemo2")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.term.claude")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-claude">✳</span> <strong>COCKPIT - 1</strong> <span class="d-muted">{$trad("docs.term.claudeDemo1")}</span></div>
          <div class="d-row"><span class="d-dot"></span> COCKPIT - 2 <span class="d-muted">{$trad("docs.term.claudeDemo2")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.term.zoom")}</p>
      </div>

    {:else if section === "fichiers"}
      <h3>{$trad("docs.files.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.files.search")}</p>
        <div class="demo">
          <div class="d-input">🔍 timeout</div>
          <div class="d-section">{$trad("docs.files.searchNames")}</div>
          <div class="d-row indent">· src/utils/timeout.ts</div>
          <div class="d-section">{$trad("docs.files.searchContent")}</div>
          <div class="d-row indent"><strong>src/api/client.ts</strong> <span class="d-count">3</span></div>
          <div class="d-row indent2"><span class="d-muted">42</span> <code>const TIMEOUT = 30_000;</code></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.files.findInFile")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-input">timeout</span><span class="d-btn small">Aa</span><span class="d-muted">3/17</span><span class="d-btn small">↑</span><span class="d-btn small">↓</span><span class="d-btn small">×</span></div>
          <div class="d-code"><span class="d-lineno">41</span>if (elapsed &gt; <mark>timeout</mark>) &#123;<br /><span class="d-lineno">42</span>  throw new <mark class="cur">Timeout</mark>Error();</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.files.manage")}</p>
        <div class="demo">
          <div class="d-menu">
            <div>{$trad("files.newFile")}</div><div>{$trad("files.newFolder")}</div><div>{$trad("common.rename")}</div><div>{$trad("files.copyPath")}</div><div class="danger">{$trad("files.trash")}</div>
          </div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.files.definition")}</p>
        <div class="demo">
          <div class="d-code"><span class="d-lineno">12</span>const user = <u class="d-link">loadUser</u>(id); <span class="d-note">{$trad("docs.files.definitionDemo")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{$trad("docs.files.header")}</p>
      </div>

      <div class="block">
        <p>{@html $trad("docs.files.images")}</p>
        <div class="demo">
          <div class="d-checker"><span class="d-imgbox">logo.png</span></div>
        </div>
      </div>

    {:else if section === "git"}
      <h3>{$trad("docs.git.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.git.changes")}</p>
        <div class="demo">
          <div class="d-section">{$trad("docs.git.changesDemo")} <span class="d-link-txt">{$trad("git.stageAll")}</span></div>
          <div class="d-row"><span class="d-badge mod">M</span> src/app.ts <span class="d-stat">+12 −3</span> <span class="d-btn small">+</span></div>
          <div class="d-row"><span class="d-badge new">?</span> notes.md <span class="d-stat">+40</span> <span class="d-btn small">+</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.git.pushPull")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small">⎇ main ▾</span><span class="d-spring"></span><span class="d-btn small">⬇ Pull <span class="d-count">2</span></span><span class="d-btn small primary">⬆ Push <span class="d-count">1</span></span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.git.history")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-tab">{$trad("git.changes")}</span><span class="d-tab active">{$trad("git.history")}</span></div>
          <div class="d-row"><strong>{$trad("docs.git.historyDemo1")}</strong></div>
          <div class="d-row indent"><code class="d-hash">8459158</code> <span class="d-muted">{$trad("docs.git.historyDemo2")}</span> <span class="d-count">main</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.git.branches")}</p>
      </div>

      <div class="block">
        <p>{@html $trad("docs.git.worktrees")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-muted">{$trad("git.changes")}</span><span class="d-muted">{$trad("git.history")}</span><span class="d-state">{$trad("git.worktrees")}</span></div>
          <div class="d-row"><strong>main</strong> <span class="d-count">{$trad("git.worktreeMain")}</span> <span class="d-btn small">▶</span></div>
          <div class="d-row indent"><span class="d-muted">/home/moi/projet</span></div>
          <div class="d-row"><strong>feat/refonte</strong> <span class="d-btn small">▶</span> <span class="d-btn small danger">🗑</span></div>
          <div class="d-row indent"><span class="d-muted">/home/moi/projet.worktrees/feat-refonte</span></div>
          <div class="d-note">{$trad("docs.git.worktreesDemo")}</div>
        </div>
      </div>

    {:else if section === "docker"}
      <h3>{$trad("docs.docker.heading")}</h3>

      <div class="block">
        <p>{$trad("docs.docker.control")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-state">RUNNING</span><span class="d-btn small ok">Start</span><span class="d-btn small danger">Stop</span><span class="d-btn small">Restart</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.docker.logs")}</p>
        <div class="demo">
          <div class="d-row">web-1 <span class="d-muted">running · 8080→80</span> <span class="d-spring"></span><span class="d-btn small">{$trad("docker.logs")}</span><span class="d-btn small">{$trad("docker.shell")}</span></div>
          <div class="d-term">GET /health 200 3ms<br />GET /api/users 200 12ms</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.docker.dashboard")}</p>
      </div>

      <div class="block">
        <p>{$trad("docs.docker.noStandardFile")}</p>
      </div>

      <div class="block">
        <p>{@html $trad("docs.docker.optional")}</p>
        <div class="demo">
          <div class="d-row d-warn">{$trad("docs.docker.optionalDemo")}</div>
          <div class="d-tabs"><span class="d-btn small off">Start</span><span class="d-btn small off">Stop</span><span class="d-btn small off">Restart</span></div>
        </div>
      </div>

    {:else if section === "taches"}
      <h3>{$trad("docs.tasks.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.tasks.todos")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-check"></span> {$trad("docs.tasks.demo1")} <span class="d-due warn">{$trad("docs.tasks.demo1Due")}</span></div>
          <div class="d-row"><span class="d-check"></span> {$trad("docs.tasks.demo2")} <span class="d-due err">{$trad("docs.tasks.demo2Due")}</span></div>
          <div class="d-row"><span class="d-check"></span> {$trad("docs.tasks.demo3")} <span class="d-due">20/08</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.tasks.progress")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-check"></span> {$trad("docs.tasks.demo1")} <span class="d-bar"><span class="d-bar-fill" style="width: 70%"></span></span> <span class="d-muted">70%</span></div>
          <div class="d-row"><span class="d-check"></span> {$trad("docs.tasks.demo2")} <span class="d-bar"><span class="d-bar-fill" style="width: 20%"></span></span> <span class="d-muted">20%</span></div>
          <div class="d-row"><span class="d-check"></span> {$trad("docs.tasks.demo3")} <span class="d-bar"></span></div>
          <div class="d-note">{$trad("docs.tasks.progressDemo")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.tasks.todoLinks")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-check"></span> {@html $trad("docs.tasks.todoLinksDemo")}</div>
          <div class="d-row"><span class="kbd">Ctrl</span> + <span class="d-muted">{$trad("docs.tasks.todoLinksClick")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.tasks.bell")}</p>
        <div class="demo">
          <div class="d-notif">⚠ <strong>{$trad("alerts.todoOverdue")}</strong><br /><span class="d-muted">{$trad("docs.tasks.bellDemo")}</span> <span class="d-btn small primary">{$trad("alerts.seeProject")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.tasks.notes")}</p>
        <p>{@html $trad("docs.tasks.notesLinks")}</p>
        <p>{@html $trad("docs.tasks.notesNormal")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small"><b>B</b></span><span class="d-btn small"><i>I</i></span><span class="d-btn small">¶</span><span class="d-btn small">H1</span><span class="d-btn small">{$trad("docs.tasks.notesDemoList")}</span><span class="d-btn small">‹/›</span></div>
          <div class="d-note">{$trad("docs.tasks.notesDemo")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.tasks.notesCode")}</p>
        <div class="demo">
          <div class="d-note">const x = 1;</div>
          <div class="d-row"><span class="kbd">{$trad("docs.shortcuts.keyEnter")}</span> <span class="kbd">{$trad("docs.shortcuts.keyEnter")}</span> <span class="d-muted">{$trad("docs.tasks.notesCodeOut")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.tasks.reading")}</p>
        <div class="demo">
          <div class="d-tabs">
            <span class="d-section">{$trad("notes.title")}</span>
            <span class="d-spring"></span>
            <span class="d-btn small"><b>B</b></span>
            <span class="d-btn small"><i>I</i></span>
            <span class="d-btn small primary">▸◂ {$trad("notes.reading")}</span>
          </div>
          <div class="d-note">{$trad("docs.tasks.readingDemo")}</div>
          <div class="d-row"><span class="kbd">{$trad("docs.shortcuts.keyEscape")}</span> <span class="d-muted">{$trad("docs.tasks.readingBack")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.tasks.meetings")}</p>
        <p>{$trad("docs.tasks.transcript")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-recdot"></span> <strong>12:34</strong> <span class="d-btn small danger">{$trad("rec.stop")}</span> <span class="d-muted">{$trad("docs.tasks.meetingsDemo")}</span></div>
        </div>
      </div>

    {:else if section === "palette"}
      <h3>{$trad("docs.palette.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.palette.commands")}</p>
        <div class="demo">
          <div class="d-menu"><div>Dev <span class="d-muted">npm run dev</span></div><div>Up <span class="d-muted">make up</span></div><div>Tests <span class="d-muted">cargo test</span></div></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.palette.palette")}</p>
        <div class="demo">
          <div class="d-input">api</div>
          <div class="d-section">{$trad("docs.palette.sectionProjects")}</div>
          <div class="d-row indent sel">api-gateway</div>
          <div class="d-section">{$trad("docs.palette.sectionFiles")}</div>
          <div class="d-row indent">src/api/client.ts</div>
          <div class="d-note">{$trad("docs.palette.demo")}</div>
        </div>
      </div>

    {:else if section === "dashboard"}
      <h3>{$trad("docs.dash.heading")}</h3>

      <div class="block">
        <p>{$trad("docs.dash.intro")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-tab active">{$trad("dashboard.tasksTab")}</span><span class="d-tab">{$trad("dashboard.monitoringTab")}</span><span class="d-tab">{$trad("dashboard.terminalsTab")}</span><span class="d-tab">{$trad("dashboard.containersTab")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.dash.tasks")}</p>
      </div>

      <div class="block">
        <p>{@html $trad("docs.dash.monitoring")}</p>
        <div class="demo">
          <div class="d-row"><span class="d-gauge">{$trad("docs.dash.monitoringDemo1")}</span><span class="d-gauge">{$trad("docs.dash.monitoringDemo2")}</span><span class="d-spring"></span><span class="d-muted">{$trad("docs.dash.monitoringDemo3")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.dash.terminals")}</p>
      </div>

      <div class="block">
        <p>{@html $trad("docs.dash.containers")}</p>
      </div>

    {:else if section === "apparence"}
      <h3>{$trad("docs.appearance.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.appearance.palettes")}</p>
        <div class="demo">
          <div class="d-tabs">
            <span class="d-swatch" style="background:#161922"></span>
            <span class="d-swatch" style="background:#111830"></span>
            <span class="d-swatch" style="background:#1d182a"></span>
            <span class="d-swatch" style="background:#121d18"></span>
            <span class="d-swatch light" style="background:#fffdf8"></span>
            <span class="d-muted">{$trad("docs.appearance.palettesDemo")}</span>
          </div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.appearance.language")}</p>
        <div class="demo">
          <div class="d-row">
            <strong>{$trad("settings.language")}</strong>
            <span class="d-btn small">Français</span><span class="d-btn small">English</span>
            <span class="d-note">{$trad("docs.appearance.languageDemo")}</span>
          </div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.appearance.zoom")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small">🔔</span><span class="d-btn small">−</span><span class="d-muted">115 %</span><span class="d-btn small">+</span><span class="d-btn small"><i>i</i></span><span class="d-btn small">⚙</span><span class="d-btn small">◑</span></div>
        </div>
      </div>

    {:else if section === "compte"}
      <h3>{$trad("docs.account.heading")}</h3>

      <div class="block">
        <p>{@html $trad("docs.account.what")}</p>
      </div>

      <div class="block">
        <p>{@html $trad("docs.account.where")}</p>
        <div class="demo">
          <div class="d-row">
            <span class="d-muted">{$trad("docs.account.headerDemo")}</span>
            <span class="d-btn small">{$trad("docs.account.initialsDemo")}</span>
          </div>
          <div class="d-note">{$trad("docs.account.menuDemo")}</div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.account.page")}</p>
        <div class="demo">
          <div class="d-row"><strong>{$trad("compte.profil.nom")}</strong><span class="d-muted">{$trad("docs.account.nameDemo")}</span></div>
          <div class="d-row"><strong>{$trad("compte.profil.synchro")}</strong><span class="d-muted">{$trad("docs.account.syncDemo")}</span></div>
          <div class="d-row"><strong>{$trad("compte.profil.machines")}</strong><span class="d-muted">{$trad("docs.account.machinesDemo")}</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.account.offline")}</p>
      </div>

    {:else if section === "maj"}
      <h3>{$trad("docs.updates.heading")}</h3>

      <div class="block">
        <p>{$trad("docs.updates.bell")}</p>
        <div class="demo">
          <div class="d-notif">⬇ <strong>{$trad("docs.updates.bellDemo1")}</strong><br /><span class="d-muted">{$trad("docs.updates.bellDemo2")}</span> <span class="d-btn small primary">{$trad("update.install")}</span></div>
          <div class="d-note">{$trad("docs.updates.bellDemo3")}</div>
        </div>
      </div>

      <div class="block">
        <p>{$trad("docs.updates.cadence")}</p>
      </div>

      <div class="block">
        <p>{@html $trad("docs.updates.backup")}</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small primary">{$trad("settings.backup.button")}</span><span class="d-muted">→ cockpit-sauvegarde-2026-08-14.db</span></div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.updates.errors")}</p>
        <div class="demo">
          <div class="d-row">
            <strong>{$trad("settings.reporting.title")}</strong>
            <span class="d-btn small">{$trad("settings.reporting.enabled")}</span>
            <span class="d-note">{$trad("docs.updates.errorsDemo")}</span>
          </div>
        </div>
      </div>

      <div class="block">
        <p>{@html $trad("docs.updates.alerts")}</p>
        <div class="demo">
          <div class="d-notif">⚠ <strong>{$trad("docs.updates.alertsDemo1")}</strong><br /><span class="d-muted">{$trad("docs.updates.alertsDemo2")}</span> <span class="d-btn small primary">{$trad("alerts.seeMonitoring")}</span></div>
        </div>
      </div>

    {:else if section === "raccourcis"}
      <h3>{$trad("docs.shortcuts.heading")}</h3>

      <div class="block">
        <table class="d-table">
          <tbody>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">K</span></td><td>{$trad("docs.shortcuts.palette")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">F</span></td><td>{$trad("docs.shortcuts.findFile")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">{$trad("docs.shortcuts.keyShift")}</span><span class="kbd">F</span></td><td>{$trad("docs.shortcuts.findProject")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span>{$trad("docs.shortcuts.ctrlClick")}</td><td>{$trad("docs.shortcuts.definition")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span>{$trad("docs.shortcuts.ctrlClick")}</td><td>{$trad("docs.shortcuts.openLink")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">S</span></td><td>{$trad("docs.shortcuts.save")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">{$trad("docs.shortcuts.keyEnter")}</span></td><td>{$trad("docs.shortcuts.commit")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">{$trad("docs.shortcuts.keyEnter")}</span></td><td>{$trad("docs.shortcuts.leaveCodeBlock")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">C</span></td><td>{$trad("docs.shortcuts.copy")}</td></tr>
            <tr><td><span class="kbd">Ctrl</span>{$trad("docs.shortcuts.ctrlWheel")}</td><td>{$trad("docs.shortcuts.zoom")}</td></tr>
            <tr><td><span class="kbd">{$trad("docs.shortcuts.keyEscape")}</span></td><td>{$trad("docs.shortcuts.escape")}</td></tr>
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<style>
  .docs { display: flex; gap: 1.25rem; max-width: 1100px; margin: 0 auto; }
  nav {
    width: 220px; flex-shrink: 0; align-self: flex-start;
    display: flex; flex-direction: column; gap: 0.15rem;
    position: sticky; top: 0;
  }
  nav h2 { margin: 0.2rem 0.6rem 0.6rem; font-size: 1rem; }
  nav button {
    display: flex; align-items: center; gap: 0.55rem; text-align: left;
    background: none; border: none; cursor: pointer;
    padding: 0.45rem 0.6rem; border-radius: var(--radius-sm, 6px);
    color: var(--text-secondary); font-size: 0.86rem;
  }
  nav button:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  nav button.active { background: var(--accent-soft); color: var(--accent); font-weight: 600; }
  nav .icon { width: 1.4rem; text-align: center; font-size: 0.85rem; }

  .content { flex: 1; min-width: 0; padding: 1.35rem 1.5rem; }
  .content h3 { margin: 0.2rem 0 1rem; font-size: 1.15rem; }
  .block { margin-bottom: 1.4rem; }
  .block > p { margin: 0 0 0.5rem; font-size: 0.88rem; color: var(--text-secondary); }
  .block strong { color: var(--text-primary); }

  /* --- Langage visuel des maquettes --- */
  .demo {
    border: 1px solid var(--border-color); border-radius: 10px;
    background: var(--bg-secondary); padding: 0.7rem 0.8rem;
    display: flex; flex-direction: column; gap: 0.4rem;
  }
  .kbd {
    display: inline-block; padding: 0.05rem 0.4rem; margin: 0 0.1rem;
    border: 1px solid var(--border-strong, var(--border-color)); border-bottom-width: 2px;
    border-radius: 5px; background: var(--bg-tertiary);
    font-family: var(--font-mono, monospace); font-size: 0.72rem; color: var(--text-primary);
  }
  .d-tabs { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .d-tab {
    padding: 0.2rem 0.6rem; font-size: 0.78rem; color: var(--text-muted);
    border-bottom: 2px solid transparent;
  }
  .d-tab.active { color: var(--accent); border-bottom-color: var(--accent); }
  .d-spring { flex: 1; }
  .d-btn {
    display: inline-flex; align-items: center; gap: 0.25rem;
    border: 1px solid var(--border-color); border-radius: 5px;
    background: var(--bg-tertiary); color: var(--text-primary);
    padding: 0.15rem 0.5rem; font-size: 0.75rem;
  }
  .d-btn.primary { background: var(--accent); border-color: var(--accent); color: white; }
  .d-btn.danger { color: var(--error); border-color: var(--error); background: none; }
  .d-btn.ok { color: var(--success); border-color: var(--success); background: none; }
  .d-btn.off { opacity: 0.45; }
  .d-input {
    border: 1px solid var(--accent); border-radius: 6px; background: var(--bg-primary);
    padding: 0.25rem 0.5rem; font-size: 0.8rem; color: var(--text-primary); max-width: 18rem;
  }
  .d-row { display: flex; align-items: center; gap: 0.45rem; font-size: 0.82rem; }
  .d-row.indent { padding-left: 1rem; }
  .d-row.indent2 { padding-left: 2rem; }
  .d-row.sel { background: var(--accent-soft); border-radius: 5px; padding: 0.15rem 0.4rem 0.15rem 1rem; }
  .d-row.d-warn {
    color: var(--warning); border: 1px solid var(--warning); border-radius: 5px;
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    padding: 0.2rem 0.45rem;
  }
  .d-section { font-size: 0.68rem; font-weight: 700; letter-spacing: 0.05em; color: var(--text-muted); }
  .d-muted { color: var(--text-muted); font-size: 0.75rem; }
  .d-count {
    background: var(--bg-tertiary); border: 1px solid var(--border-color);
    border-radius: 8px; padding: 0 0.35rem; font-size: 0.68rem; color: var(--text-secondary);
  }
  .d-note { color: var(--text-muted); font-size: 0.75rem; font-style: italic; }
  .d-caret { color: var(--text-muted); }
  .d-hover { margin-left: auto; opacity: 0.6; font-size: 0.75rem; }
  .d-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--text-muted); flex-shrink: 0; }
  .d-dot.ok { background: var(--success); }
  .d-dot.err { background: var(--error); }
  .d-claude { color: #d97757; font-weight: 700; }
  .d-side-head { display: flex; align-items: center; gap: 0.4rem; font-size: 0.75rem; font-weight: 700; letter-spacing: 0.05em; color: var(--text-muted); }
  .d-side-head .d-btn { margin-left: 0.2rem; }
  .d-term {
    background: #0d1117; color: #d0d7de; border-radius: 7px;
    font-family: var(--font-mono, monospace); font-size: 0.76rem; line-height: 1.6;
    padding: 0.5rem 0.7rem; position: relative;
  }
  .d-term mark { background: rgba(88, 166, 255, 0.45); color: inherit; border-radius: 2px; }
  .d-float { position: absolute; top: 0.3rem; right: 0.5rem; color: #8b949e; font-size: 0.7rem; }
  .d-code {
    font-family: var(--font-mono, monospace); font-size: 0.78rem; line-height: 1.7;
    color: var(--text-primary);
  }
  .d-code mark { background: color-mix(in srgb, var(--warning) 38%, transparent); color: inherit; border-radius: 2px; }
  .d-code mark.cur { background: color-mix(in srgb, var(--accent) 55%, transparent); outline: 1px solid var(--accent); }
  .d-lineno { display: inline-block; width: 2.2em; color: var(--text-muted); opacity: 0.6; }
  .d-link { text-decoration-color: var(--accent); color: var(--accent); }
  .d-link-txt { color: var(--accent); font-size: 0.72rem; margin-left: auto; }
  .d-menu {
    border: 1px solid var(--border-color); border-radius: 7px; background: var(--surface-base);
    box-shadow: var(--shadow-lg, 0 8px 24px rgba(0,0,0,0.25));
    padding: 0.25rem; max-width: 16rem; font-size: 0.8rem;
  }
  .d-menu div { display: flex; justify-content: space-between; gap: 0.6rem; padding: 0.3rem 0.5rem; border-radius: 4px; }
  .d-menu div:hover { background: var(--bg-tertiary); }
  .d-menu .danger { color: var(--error); }
  .d-menu.indent { margin-left: 1.5rem; }
  /* Crayon de la maquette : meme discretion que le vrai (visible au survol du titre). */
  .d-pencil { color: var(--text-muted); font-size: 0.75rem; }
  .d-badge { font-family: var(--font-mono, monospace); font-weight: 700; font-size: 0.75rem; width: 1rem; text-align: center; }
  .d-badge.mod { color: #d29922; }
  .d-badge.new { color: #6e9fff; }
  .d-stat { font-family: var(--font-mono, monospace); font-size: 0.68rem; color: var(--text-muted); margin-left: auto; }
  .d-hash { color: var(--accent); font-size: 0.75rem; }
  .d-state { color: var(--success); font-weight: 700; font-size: 0.78rem; }
  .d-check { width: 15px; height: 15px; border: 2px solid var(--text-muted); border-radius: 50%; flex-shrink: 0; }
  /* Barre d'avancement des maquettes : meme langage visuel que le reste de la doc. */
  .d-bar {
    display: inline-block; width: 3.5rem; height: 5px; border-radius: 3px;
    background: var(--bg-tertiary); overflow: hidden; vertical-align: middle;
  }
  .d-bar-fill { display: block; height: 100%; background: var(--accent); }
  .d-due {
    margin-left: auto; border: 1px solid var(--border-color); border-radius: 10px;
    padding: 0.05rem 0.45rem; font-size: 0.68rem; color: var(--text-secondary);
  }
  .d-due.warn { border-color: var(--warning); color: var(--warning); }
  .d-due.err { border-color: var(--error); color: var(--error); }
  .d-notif {
    border: 1px solid var(--border-color); border-left: 3px solid var(--accent);
    border-radius: 7px; padding: 0.5rem 0.7rem; font-size: 0.8rem; line-height: 1.7;
    background: var(--surface-base);
  }
  .d-recdot { width: 9px; height: 9px; border-radius: 50%; background: var(--error); animation: docpulse 1.2s infinite; }
  @keyframes docpulse { 50% { opacity: 0.25; } }
  .d-swatch { width: 26px; height: 26px; border-radius: 7px; border: 1px solid var(--border-color); }
  .d-checker {
    display: flex; align-items: center; justify-content: center; padding: 1rem;
    background:
      repeating-conic-gradient(color-mix(in srgb, var(--text-muted) 12%, transparent) 0% 25%, transparent 0% 50%)
      50% / 18px 18px;
    border-radius: 7px;
  }
  .d-imgbox {
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-secondary); padding: 1.2rem 2.2rem; font-size: 0.78rem;
    color: var(--text-muted);
  }
  .d-gauge {
    display: inline-flex; align-items: center; justify-content: center;
    border: 3px solid var(--accent); border-radius: 50%;
    width: 58px; height: 58px; font-size: 0.62rem; font-weight: 700;
    color: var(--text-primary); text-align: center;
  }
  .d-swatch.light { border-color: #ccc; }
  .d-table { border-collapse: collapse; font-size: 0.85rem; }
  .d-table td { padding: 0.35rem 1rem 0.35rem 0; color: var(--text-secondary); }
  .d-table td:first-child { white-space: nowrap; }
</style>
