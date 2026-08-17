<script lang="ts">
  /**
   * Documentation integree (bouton ⓘ du Header). Principe voulu par Jimmy :
   * TRES PEU de texte, surtout des exemples et des illustrations. Les "captures"
   * sont des maquettes dessinees en HTML/CSS : legeres, nettes a tout zoom, et
   * elles suivent le theme. Chaque bloc = une legende d'une ligne + une maquette.
   */
  type SectionId =
    | "demarrer" | "terminaux" | "fichiers" | "git" | "docker"
    | "taches" | "palette" | "dashboard" | "apparence" | "maj" | "raccourcis";

  const MENU: { id: SectionId; label: string; icon: string }[] = [
    { id: "demarrer", label: "Démarrer", icon: "🚀" },
    { id: "dashboard", label: "Tableau de bord", icon: "📊" },
    { id: "terminaux", label: "Terminaux", icon: ">_" },
    { id: "fichiers", label: "Fichiers", icon: "📄" },
    { id: "git", label: "Git", icon: "⎇" },
    { id: "docker", label: "Docker", icon: "🐳" },
    { id: "taches", label: "Tâches & notes", icon: "✓" },
    { id: "palette", label: "Commandes & palette", icon: "⌘" },
    { id: "apparence", label: "Apparence & zoom", icon: "🎨" },
    { id: "maj", label: "Mises à jour", icon: "🔔" },
    { id: "raccourcis", label: "Raccourcis", icon: "⌨" },
  ];

  let section: SectionId = $state("demarrer");
</script>

<div class="docs">
  <nav>
    <h2>Documentation</h2>
    {#each MENU as m (m.id)}
      <button class:active={section === m.id} onclick={() => (section = m.id)}>
        <span class="icon">{m.icon}</span>{m.label}
      </button>
    {/each}
  </nav>

  <!-- .stack : panneau continu (fond translucide sous wallpaper via components.css) —
       sans lui, titres et legendes flottaient directement sur l'image de fond. -->
  <div class="content stack">
    {#if section === "demarrer"}
      <h3>🚀 Démarrer</h3>

      <div class="block">
        <p>Un projet Cockpit, c'est <strong>un nom et un dossier</strong>. Docker est optionnel.</p>
        <div class="demo">
          <div class="d-side-head">PROJETS <span class="d-btn small">+ Projet</span> <span class="d-btn small">+ Dossier</span></div>
          <div class="d-note">👉 « + Projet » ouvre le formulaire : seul le nom est obligatoire.</div>
        </div>
      </div>

      <div class="block">
        <p>Range tes projets dans des dossiers : glisse-dépose, double-clic pour renommer, corbeille au survol (dossier vide uniquement).</p>
        <div class="demo">
          <div class="d-row"><span class="d-caret">▾</span> <strong>Core</strong> <span class="d-count">12</span> <span class="d-hover">🗑</span></div>
          <div class="d-row indent"><span class="d-dot ok"></span> api-gateway <span class="d-muted">running</span></div>
          <div class="d-row indent"><span class="d-dot"></span> worker <span class="d-muted">stopped</span></div>
        </div>
      </div>

      <div class="block">
        <p>La barre du projet réunit tout : onglets, liens rapides, commandes ▶ et l'enregistreur de réunions.</p>
        <div class="demo">
          <div class="d-tabs">
            <strong>mon-projet</strong>
            <span class="d-tab active">Workspace</span><span class="d-tab">Docker</span><span class="d-tab">Terminal</span><span class="d-tab">Fichiers</span><span class="d-tab">Git</span>
            <span class="d-spring"></span>
            <span class="d-btn small">▶ Cmd</span><span class="d-btn small danger">⏺ Enregistrer</span>
          </div>
        </div>
      </div>

      <div class="block">
        <p><strong>Liens rapides surveillés</strong> : chaque URL du projet (Paramètres → URLs) porte une pastille — vert en ligne, rouge injoignable, re-vérifiée chaque minute.</p>
        <div class="demo">
          <div class="d-tabs">
            <span class="d-btn small"><span class="d-dot ok"></span> Préprod</span>
            <span class="d-btn small"><span class="d-dot err"></span> Staging</span>
            <span class="d-note">👉 survole pour voir le code HTTP ou l'erreur</span>
          </div>
        </div>
      </div>

    {:else if section === "terminaux"}
      <h3>&gt;_ Terminaux</h3>

      <div class="block">
        <p><strong>Persistants</strong> : ferme Cockpit, redémarre — tes terminaux et ce qui tourne dedans sont toujours là.</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-tab active">MON-PROJET - 1 ×</span><span class="d-tab">MON-PROJET - 2 ×</span><span class="d-btn small">+</span><span class="d-btn small">✳ Claude ▾</span><span class="d-btn small">🔍</span></div>
          <div class="d-term">$ npm run dev<br />VITE ready in 320 ms ➜ http://localhost:5173</div>
          <div class="d-note">👉 double-clic sur l'onglet = renommer · le nom apparaît aussi dans la sidebar</div>
        </div>
      </div>

      <div class="block">
        <p><strong>Recherche dans l'historique</strong> : 🔍 ou <span class="kbd">Ctrl</span><span class="kbd">Maj</span><span class="kbd">F</span>, Entrée cherche vers le haut, surlignage et compteur dans le terminal.</p>
        <div class="demo">
          <div class="d-term">…<br />ERROR connection <mark>timeout</mark> after 30s <span class="d-float">(1/4)</span><br />retrying…</div>
        </div>
      </div>

      <div class="block">
        <p><strong>Copier / coller</strong> : sélection à la souris puis <span class="kbd">Ctrl</span><span class="kbd">C</span>, ou clic droit → Copier / Coller. <span class="kbd">Maj</span>+glisser pour sélectionner dans vim/claude.</p>
        <div class="demo">
          <div class="d-term">$ cat config.yml<br /><mark>database_url: postgres://…</mark> ← sélection bleue<br />$</div>
        </div>
      </div>

      <div class="block">
        <p><strong>Glisser-déposer un fichier</strong> : lâche une image (ou n'importe quel fichier) sur le terminal, son chemin s'écrit à l'invite — de quoi donner une capture d'écran à Claude sans taper le chemin.</p>
        <div class="demo">
          <div class="d-term">&gt; regarde cette capture /home/moi/Images/bug.png ▌</div>
          <div class="d-note">👉 le cadre du terminal s'allume pendant le survol</div>
        </div>
      </div>

      <div class="block">
        <p><strong>Claude Code intégré</strong> : le bouton ✳ Claude liste tes conversations passées du projet — un clic les reprend dans un terminal. Le logo Claude s'affiche quand un agent IA tourne.</p>
        <div class="demo">
          <div class="d-row"><span class="d-claude">✳</span> <strong>COCKPIT - 1</strong> <span class="d-muted">agent IA actif</span></div>
          <div class="d-row"><span class="d-dot"></span> COCKPIT - 2 <span class="d-muted">shell normal</span></div>
        </div>
      </div>

      <div class="block">
        <p><span class="kbd">Ctrl</span>+molette = zoom, partout, terminal compris.</p>
      </div>

    {:else if section === "fichiers"}
      <h3>📄 Fichiers</h3>

      <div class="block">
        <p><strong>Recherche globale</strong> (<span class="kbd">Ctrl</span><span class="kbd">Maj</span><span class="kbd">F</span>) : noms de dossiers, de fichiers ET contenu. Clic = ouverture sur la ligne.</p>
        <div class="demo">
          <div class="d-input">🔍 timeout</div>
          <div class="d-section">NOMS · 2</div>
          <div class="d-row indent">· src/utils/timeout.ts</div>
          <div class="d-section">CONTENU · 7</div>
          <div class="d-row indent"><strong>src/api/client.ts</strong> <span class="d-count">3</span></div>
          <div class="d-row indent2"><span class="d-muted">42</span> <code>const TIMEOUT = 30_000;</code></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Recherche dans le fichier</strong> (<span class="kbd">Ctrl</span><span class="kbd">F</span>) : compteur, Entrée / <span class="kbd">Maj</span>+Entrée pour naviguer, Aa pour la casse.</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-input">timeout</span><span class="d-btn small">Aa</span><span class="d-muted">3/17</span><span class="d-btn small">↑</span><span class="d-btn small">↓</span><span class="d-btn small">×</span></div>
          <div class="d-code"><span class="d-lineno">41</span>if (elapsed &gt; <mark>timeout</mark>) &#123;<br /><span class="d-lineno">42</span>  throw new <mark class="cur">Timeout</mark>Error();</div>
        </div>
      </div>

      <div class="block">
        <p><strong>Gérer les fichiers</strong> : clic droit sur l'arbre. La suppression va à la <strong>corbeille système</strong>, jamais définitive.</p>
        <div class="demo">
          <div class="d-menu">
            <div>Nouveau fichier</div><div>Nouveau dossier</div><div>Renommer</div><div>Copier le chemin</div><div class="danger">Mettre à la corbeille</div>
          </div>
        </div>
      </div>

      <div class="block">
        <p><strong>Aller à la définition</strong> : <span class="kbd">Ctrl</span>+clic sur un symbole (LSP si installé, repli automatique sinon). <strong>Éditer</strong> : bouton ✎ puis <span class="kbd">Ctrl</span><span class="kbd">S</span>.</p>
        <div class="demo">
          <div class="d-code"><span class="d-lineno">12</span>const user = <u class="d-link">loadUser</u>(id); <span class="d-note">← Ctrl+clic saute à la déclaration</span></div>
        </div>
      </div>

      <div class="block">
        <p>Dans l'en-tête : total de lignes et taille, ⧉ copie le chemin, ⏎ active le retour à la ligne (logs, Markdown).</p>
      </div>

      <div class="block">
        <p><strong>Les images s'affichent</strong> (png, jpg, webp, gif…) sur un damier qui révèle la transparence.</p>
        <div class="demo">
          <div class="d-checker"><span class="d-imgbox">logo.png</span></div>
        </div>
      </div>

    {:else if section === "git"}
      <h3>⎇ Git</h3>

      <div class="block">
        <p><strong>Modifications</strong> : indexe fichier par fichier (+/−) ou tout d'un coup, diff coloré à droite, commit avec <span class="kbd">Ctrl</span><span class="kbd">Entrée</span>.</p>
        <div class="demo">
          <div class="d-section">MODIFICATIONS (2) <span class="d-link-txt">Tout indexer</span></div>
          <div class="d-row"><span class="d-badge mod">M</span> src/app.ts <span class="d-stat">+12 −3</span> <span class="d-btn small">+</span></div>
          <div class="d-row"><span class="d-badge new">?</span> notes.md <span class="d-stat">+40</span> <span class="d-btn small">+</span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Pull / Push</strong> avec compteurs de retard/avance. Pull toujours en avance rapide : jamais de merge surprise.</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small">⎇ main ▾</span><span class="d-spring"></span><span class="d-btn small">⬇ Pull <span class="d-count">2</span></span><span class="d-btn small primary">⬆ Push <span class="d-count">1</span></span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Historique</strong> : les 100 derniers commits, clic = diff complet du commit.</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-tab">Modifications</span><span class="d-tab active">Historique</span></div>
          <div class="d-row"><strong>Corriger la detection docker</strong></div>
          <div class="d-row indent"><code class="d-hash">8459158</code> <span class="d-muted">Jimmy · il y a 2 h</span> <span class="d-count">main</span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Branches</strong> : le bouton ⎇ change de branche, en crée, en supprime.</p>
      </div>

    {:else if section === "docker"}
      <h3>🐳 Docker</h3>

      <div class="block">
        <p>Start / Stop / Restart du projet, avec ses dépendances démarrées dans le bon ordre.</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-state">RUNNING</span><span class="d-btn small ok">Start</span><span class="d-btn small danger">Stop</span><span class="d-btn small">Restart</span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Logs</strong> en direct (rafraîchis toutes les 2 s) et <strong>Shell</strong> dans le conteneur, depuis le tableau des conteneurs.</p>
        <div class="demo">
          <div class="d-row">web-1 <span class="d-muted">running · 8080→80</span> <span class="d-spring"></span><span class="d-btn small">Logs</span><span class="d-btn small">Shell</span></div>
          <div class="d-term">GET /health 200 3ms<br />GET /api/users 200 12ms</div>
        </div>
      </div>

      <div class="block">
        <p>Le tableau de bord → Conteneurs montre <strong>tous</strong> les conteneurs de la machine, avec volumes, images et nettoyage (prune).</p>
      </div>

      <div class="block">
        <p>Pas de fichier compose au nom standard ? Cockpit retrouve quand même les conteneurs lancés depuis le dossier du projet. Et si Docker est en panne, l'onglet affiche la cause exacte.</p>
      </div>

      <div class="block">
        <p>Le fichier compose est <strong>optionnel</strong> : sans lui, Start / Stop restent grisés et l'onglet dit où en poser un, ou comment nommer le vôtre.</p>
        <div class="demo">
          <div class="d-row d-warn">Aucun fichier compose trouvé dans /srv/mon-projet</div>
          <div class="d-tabs"><span class="d-btn small off">Start</span><span class="d-btn small off">Stop</span><span class="d-btn small off">Restart</span></div>
        </div>
      </div>

    {:else if section === "taches"}
      <h3>✓ Tâches & notes</h3>

      <div class="block">
        <p><strong>Tâches</strong> par projet : coche, édite au clic, glisse pour réordonner. 📅 au survol pose une <strong>échéance</strong>.</p>
        <div class="demo">
          <div class="d-row"><span class="d-check"></span> Déployer la préprod <span class="d-due warn">aujourd'hui</span></div>
          <div class="d-row"><span class="d-check"></span> Relire la doc API <span class="d-due err">en retard de 2 j</span></div>
          <div class="d-row"><span class="d-check"></span> Ranger le backlog <span class="d-due">20/08</span></div>
        </div>
      </div>

      <div class="block">
        <p>La <strong>cloche 🔔 prévient</strong> quand une tâche arrive à échéance — la notification disparaît toute seule quand la tâche est cochée.</p>
        <div class="demo">
          <div class="d-notif">⚠ <strong>Tâche en retard</strong><br /><span class="d-muted">mon-projet — Relire la doc API (hier)</span> <span class="d-btn small primary">Voir le projet</span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Notes</strong> : arborescence de dossiers + éditeur riche (gras, titres, listes, code…), sauvegarde automatique.</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small"><b>B</b></span><span class="d-btn small"><i>I</i></span><span class="d-btn small">H1</span><span class="d-btn small">• liste</span><span class="d-btn small">‹/›</span></div>
          <div class="d-note">👉 tape directement, c'est enregistré une seconde plus tard</div>
        </div>
      </div>

      <div class="block">
        <p><strong>Réunions</strong> : ⏺ Enregistrer capte micro + son système, transcrit, résume, et crée la note « Réunion du … » toute seule (clé OpenAI dans Paramètres → Réunions).</p>
        <div class="demo">
          <div class="d-row"><span class="d-recdot"></span> <strong>12:34</strong> <span class="d-btn small danger">⏹ Stop</span> <span class="d-muted">→ Transcription… → Résumé… → 📝 note créée</span></div>
        </div>
      </div>

    {:else if section === "palette"}
      <h3>⌘ Commandes & palette</h3>

      <div class="block">
        <p><strong>Commandes rapides</strong> : déclare tes commandes dans Paramètres du projet, lance-les depuis ▶ Cmd — chacune dans un nouveau terminal.</p>
        <div class="demo">
          <div class="d-menu"><div>Dev <span class="d-muted">npm run dev</span></div><div>Up <span class="d-muted">make up</span></div><div>Tests <span class="d-muted">cargo test</span></div></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Palette</strong> (<span class="kbd">Ctrl</span><span class="kbd">K</span>) : tape quelques lettres, saute vers un projet, un terminal, un onglet, un fichier ou une commande.</p>
        <div class="demo">
          <div class="d-input">api</div>
          <div class="d-section">PROJETS</div>
          <div class="d-row indent sel">api-gateway</div>
          <div class="d-section">FICHIERS</div>
          <div class="d-row indent">src/api/client.ts</div>
          <div class="d-note">👉 dans un terminal, Ctrl+K reste au shell : clique ailleurs d'abord</div>
        </div>
      </div>

    {:else if section === "dashboard"}
      <h3>📊 Tableau de bord</h3>

      <div class="block">
        <p>Quatre vues, accessibles dès l'ouverture (clic sur « Cockpit » en haut à gauche pour y revenir).</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-tab active">✓ Tâches</span><span class="d-tab">📈 Monitoring</span><span class="d-tab">&gt;_ Terminaux</span><span class="d-tab">🐳 Conteneurs</span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Tâches</strong> : toutes les tâches en attente, groupées par projet — édite, coche, déplace d'un projet à l'autre par glisser-déposer.</p>
      </div>

      <div class="block">
        <p><strong>Monitoring</strong> : CPU et mémoire en jauges + historique, détail mémoire (cache, ZFS…), disques, et le top des processus — tuables d'un clic.</p>
        <div class="demo">
          <div class="d-row"><span class="d-gauge">CPU 34 %</span><span class="d-gauge">Mém 62 %</span><span class="d-spring"></span><span class="d-muted">top 20 processus · kill au survol</span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Terminaux</strong> : toutes les sessions ouvertes, tous projets confondus — un clic y va directement.</p>
      </div>

      <div class="block">
        <p><strong>Conteneurs</strong> : tous les conteneurs Docker de la machine (pas seulement ceux de Cockpit), avec volumes, images, espace disque et boutons de nettoyage.</p>
      </div>

    {:else if section === "apparence"}
      <h3>🎨 Apparence & zoom</h3>

      <div class="block">
        <p>Paramètres → Apparence : <strong>palettes de couleurs</strong>, accent, et <strong>image de fond</strong> — voile, flou de l'image et opacité des panneaux réglables.</p>
        <div class="demo">
          <div class="d-tabs">
            <span class="d-swatch" style="background:#161922"></span>
            <span class="d-swatch" style="background:#111830"></span>
            <span class="d-swatch" style="background:#1d182a"></span>
            <span class="d-swatch" style="background:#121d18"></span>
            <span class="d-swatch light" style="background:#fffdf8"></span>
            <span class="d-muted">+ image de fond 🖼</span>
          </div>
        </div>
      </div>

      <div class="block">
        <p>◑ dans l'en-tête bascule sombre/clair. Zoom : − / + en haut à droite ou <span class="kbd">Ctrl</span>+molette (terminaux compris, toujours nets).</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small">🔔</span><span class="d-btn small">−</span><span class="d-muted">115 %</span><span class="d-btn small">+</span><span class="d-btn small"><i>i</i></span><span class="d-btn small">⚙</span><span class="d-btn small">◑</span></div>
        </div>
      </div>

    {:else if section === "maj"}
      <h3>🔔 Mises à jour & notifications</h3>

      <div class="block">
        <p>La cloche est le point d'entrée unique : mises à jour, tâches à échéance… Le badge compte les non-lues.</p>
        <div class="demo">
          <div class="d-notif">⬇ <strong>Cockpit 0.20.0 disponible</strong><br /><span class="d-muted">Historique Git, bouton Pull…</span> <span class="d-btn small primary">Mettre à jour</span></div>
          <div class="d-note">👉 télécharge, installe et relance tout seul — les notes de version sont le changelog</div>
        </div>
      </div>

      <div class="block">
        <p>Vérification au démarrage, toutes les heures, et au retour sur la fenêtre. La version installée et le changelog complet sont dans Paramètres → Général.</p>
      </div>

      <div class="block">
        <p><strong>Sauvegarde</strong> : Paramètres → Général → « Exporter la base… » écrit une copie de toutes tes données (projets, notes, tâches…) dans un fichier, où tu veux.</p>
        <div class="demo">
          <div class="d-tabs"><span class="d-btn small primary">Exporter la base…</span><span class="d-muted">→ cockpit-sauvegarde-2026-08-14.db</span></div>
        </div>
      </div>

      <div class="block">
        <p><strong>Alertes système</strong> : disque presque plein, mémoire ou CPU saturés plusieurs minutes → la cloche prévient, et l'alerte se retire seule quand ça redescend.</p>
        <div class="demo">
          <div class="d-notif">⚠ <strong>Disque presque plein — /home</strong><br /><span class="d-muted">93 % utilisés, 12,4 Go libres</span> <span class="d-btn small primary">Voir le monitoring</span></div>
        </div>
      </div>

    {:else if section === "raccourcis"}
      <h3>⌨ Raccourcis</h3>

      <div class="block">
        <table class="d-table">
          <tbody>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">K</span></td><td>Palette : aller à…</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">F</span></td><td>Chercher dans le fichier ouvert</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">Maj</span><span class="kbd">F</span></td><td>Chercher dans le projet / dans l'historique du terminal</td></tr>
            <tr><td><span class="kbd">Ctrl</span>+clic</td><td>Aller à la définition (Fichiers)</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">S</span></td><td>Sauvegarder le fichier en édition</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">Entrée</span></td><td>Commit (zone de message Git)</td></tr>
            <tr><td><span class="kbd">Ctrl</span><span class="kbd">C</span></td><td>Terminal : copier la sélection (sinon SIGINT normal)</td></tr>
            <tr><td><span class="kbd">Ctrl</span>+molette</td><td>Zoom global</td></tr>
            <tr><td><span class="kbd">Échap</span></td><td>Fermer recherche / modal / palette</td></tr>
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
  .d-badge { font-family: var(--font-mono, monospace); font-weight: 700; font-size: 0.75rem; width: 1rem; text-align: center; }
  .d-badge.mod { color: #d29922; }
  .d-badge.new { color: #6e9fff; }
  .d-stat { font-family: var(--font-mono, monospace); font-size: 0.68rem; color: var(--text-muted); margin-left: auto; }
  .d-hash { color: var(--accent); font-size: 0.75rem; }
  .d-state { color: var(--success); font-weight: 700; font-size: 0.78rem; }
  .d-check { width: 15px; height: 15px; border: 2px solid var(--text-muted); border-radius: 50%; flex-shrink: 0; }
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
