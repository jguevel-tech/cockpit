<script lang="ts">
  import { untrack } from "svelte";
  import { marked } from "marked";
  import TurndownService from "turndown";
  import { saveNoteFile } from "../../api/storage";
  import { notify } from "../../stores/toast";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import type { NoteFile } from "../../types";
  import { trad, translate } from "../../i18n";
  import { openUrl } from "../../api/workspace";
  import { signalerErreur } from "../../stores/errors";

  let {
    file,
    onRename,
  }: {
    file: NoteFile;
    onRename?: (name: string) => void;
  } = $props();

  const turndown = new TurndownService({ headingStyle: "atx", codeBlockStyle: "fenced" });

  /// Texte d'un bloc, les `<br>` comptes comme de vrais sauts de ligne.
  function texteDeBloc(node: Node): string {
    let texte = "";
    for (const enfant of Array.from(node.childNodes)) {
      if (enfant.nodeType === Node.TEXT_NODE) texte += enfant.nodeValue ?? "";
      else if (enfant.nodeName === "BR") texte += "\n";
      else texte += texteDeBloc(enfant);
    }
    return texte;
  }

  /// Tout `<pre>` redevient un bloc de code Markdown, avec ou sans enfant `<code>`.
  ///
  /// La regle d'origine de turndown lit `node.firstChild.textContent` : elle ignore donc un
  /// `<pre>` NU (celui que posait le bouton) et perd les `<br>` que WebKit intercale quand on
  /// met plusieurs lignes en bloc de code. Dans les deux cas le bloc repartait en simple
  /// paragraphe a la sauvegarde — du code perdu en silence.
  turndown.addRule("blocDeCode", {
    filter: "pre",
    replacement: (_contenu, node) => {
      const texte = texteDeBloc(node).replace(/\n+$/, "");
      const langue = (node.querySelector("code")?.className.match(/language-(\S+)/) ?? ["", ""])[1];
      // La cloture doit etre plus longue que la plus longue suite d'accents graves du contenu.
      const plusLongue = (texte.match(/`+/g) ?? []).reduce((max, suite) => Math.max(max, suite.length), 0);
      const cloture = "`".repeat(Math.max(3, plusLongue + 1));
      return `\n\n${cloture}${langue}\n${texte}\n${cloture}\n\n`;
    },
  });

  /// Blocs qui, en dernier dans la note, n'offrent AUCUNE position de caret apres eux.
  ///
  /// Mesure dans le WebKitGTK de Tauri : sous un tel bloc final, ni la fleche bas, ni la fleche
  /// droite, ni un clic au ras du bas, ni meme un `Range` force apres le bloc ne sortent de la —
  /// il n'y a rien apres lui, donc rien a viser. La note etait verrouillee.
  const BLOCS_TERMINAUX = ["PRE", "BLOCKQUOTE", "UL", "OL", "TABLE"];

  /// Blocs porteurs d'une mise en forme que le bouton ¶ sait defaire.
  const BLOCS_DE_TEXTE = "h1,h2,h3,h4,h5,h6,blockquote,pre,li,p,div";
  /// Conteneurs a SCINDER pour en sortir un paragraphe : une liste ou une citation ne peut
  /// pas contenir de texte normal, il faut couper autour du bloc qu'on remet a plat.
  const CONTENEURS_A_SCINDER = ["UL", "OL", "BLOCKQUOTE", "LI"];

  let markdownContent = $state("");
  let editorEl: HTMLDivElement | undefined = $state(undefined);
  let renaming = $state(false);
  /// Ctrl enfonce : les liens deviennent cliquables et le curseur le montre.
  let ctrlEnfonce = $state(false);

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
      if (!editorEl) return;
      editorEl.innerHTML = marked.parse(markdownContent) as string;
      // Pas de `dirty` ici : un paragraphe vide en fin de note ne produit aucun Markdown.
      garantirParagrapheFinal();
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

  /// Ouvre un lien de la note dans le navigateur du systeme.
  ///
  /// Ctrl+clic et non clic simple : la zone est un editeur, un clic doit pouvoir placer le
  /// curseur pour corriger le texte d'un lien. C'est aussi le geste deja utilise ailleurs
  /// dans Cockpit (terminal, onglet Fichiers) et dans les editeurs en general.
  async function onEditorClick(e: MouseEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    const lien = (e.target as HTMLElement | null)?.closest("a");
    const href = lien?.getAttribute("href");
    if (!href) return;
    e.preventDefault();

    // Une note peut contenir n'importe quoi (collage, import) : on n'ouvre que des adresses
    // ABSOLUES a schema sans danger. Un refus est DIT, jamais silencieux.
    const cible = analyserLien(href);
    if (cible === "incomplet") {
      notify(translate("note.linkIncomplete"), "info", 5000, { report: false });
      return;
    }
    if (cible === "illisible") {
      notify(translate("note.linkInvalid", { href }), "error", 4000, { scope: "notes.lien" });
      return;
    }
    if (!["http:", "https:", "mailto:"].includes(cible.protocol)) {
      notify(translate("note.linkRefused"), "info", 5000, { report: false });
      return;
    }
    try {
      // L'adresse ABSOLUE, pas le href brut : c'est la seule que le systeme sait ouvrir.
      await openUrl(cible.href);
    } catch (err) {
      signalerErreur("notes.ouvrirLien", String(err));
      notify(String(err), "error", 4000, { report: false });
    }
  }

  /// Trie un href de note en trois cas : adresse absolue, lien incomplet (relatif ou sans
  /// schema), href illisible.
  ///
  /// La distinction compte parce que le message affiche en depend. Resoudre le href contre une
  /// base bidon (`new URL(href, "http://note.invalid")`) faisait passer `[x](www.ex.com)` et
  /// `[x](../doc.md)` pour des liens http valides : la liste blanche les acceptait, puis le
  /// backend refusait le href brut et l'utilisateur recevait une erreur technique.
  function analyserLien(href: string): URL | "incomplet" | "illisible" {
    try {
      return new URL(href);
    } catch {
      // Pas absolu. Silence VOLONTAIRE : le cas est traite juste en dessous, pas avale.
    }
    try {
      new URL(href, "http://note.invalid");
      return "incomplet";
    } catch {
      return "illisible";
    }
  }

  /// Selection courante, seulement si elle se trouve DANS la zone d'edition.
  function selectionDansEditeur(): Selection | null {
    const sel = document.getSelection();
    if (!sel || sel.rangeCount === 0 || !editorEl) return null;
    return editorEl.contains(sel.anchorNode) ? sel : null;
  }

  /// Le bloc de code qui contient le caret, ou null.
  function blocDeCodeCourant(): HTMLElement | null {
    const sel = selectionDansEditeur();
    if (!sel) return null;
    let n: Node | null = sel.anchorNode;
    while (n && n !== editorEl) {
      if (n.nodeName === "PRE") return n as HTMLElement;
      n = n.parentNode;
    }
    return null;
  }

  function paragrapheVide(): HTMLParagraphElement {
    const p = document.createElement("p");
    p.appendChild(document.createElement("br"));
    return p;
  }

  function poserSelection(r: Range) {
    const sel = document.getSelection();
    sel?.removeAllRanges();
    sel?.addRange(r);
  }

  /// La note finit TOUJOURS par un paragraphe. C'est la cible de caret qui manquait sous un bloc
  /// final : sans elle, un bloc de code en fin de note enfermait la saisie (issue #5).
  function garantirParagrapheFinal() {
    if (!editorEl) return;
    // Le rendu Markdown laisse un saut de ligne apres le dernier bloc. Ce n'est pas une cible de
    // caret, et le prendre pour le dernier noeud faisait sauter la garde : mesure au banc, la
    // note restait verrouillee des qu'elle etait RELUE depuis le Markdown — donc pour toujours.
    let dernier = editorEl.lastChild;
    while (dernier && dernier.nodeType === Node.TEXT_NODE && !dernier.nodeValue?.trim()) {
      dernier = dernier.previousSibling;
    }
    if (!dernier || dernier.nodeType !== Node.ELEMENT_NODE) return;
    if (!BLOCS_TERMINAUX.includes(dernier.nodeName)) return;
    editorEl.appendChild(paragrapheVide());
  }

  /// Position du caret dans `bloc`, comptee en caracteres.
  function decalageCaret(bloc: HTMLElement): number {
    const sel = selectionDansEditeur();
    if (!sel) return 0;
    const courant = sel.getRangeAt(0);
    const r = document.createRange();
    r.selectNodeContents(bloc);
    r.setEnd(courant.endContainer, courant.endOffset);
    return r.toString().length;
  }

  /// Repose le caret a `position` caracteres du debut de `bloc`.
  ///
  /// Necessaire parce qu'envelopper le contenu dans un `<code>` le REPARENTE : mesure au banc,
  /// le caret retombait alors au tout debut du bloc et la frappe s'inserait avant le texte.
  function poserCaret(bloc: HTMLElement, position: number) {
    const r = document.createRange();
    const marche = document.createTreeWalker(bloc, NodeFilter.SHOW_TEXT);
    let vus = 0;
    while (marche.nextNode()) {
      const texte = marche.currentNode as Text;
      if (vus + texte.length >= position) {
        r.setStart(texte, position - vus);
        r.collapse(true);
        poserSelection(r);
        return;
      }
      vus += texte.length;
    }
    r.selectNodeContents(bloc);
    r.collapse(false);
    poserSelection(r);
  }

  /// Un `<pre>` sans enfant `<code>` n'est pas du code pour Markdown : on enveloppe toujours.
  function envelopperDansCode(pre: HTMLElement) {
    const position = decalageCaret(pre);
    let code = pre.querySelector("code");
    if (!code) {
      code = document.createElement("code");
      while (pre.firstChild) code.appendChild(pre.firstChild);
      pre.appendChild(code);
    }
    // WebKit separe par des `<br>` les lignes qu'il regroupe dans le bloc. Dans du code un saut
    // de ligne s'ecrit "\n", sinon les lignes se recollent a la sauvegarde.
    for (const br of Array.from(code.querySelectorAll("br"))) {
      br.replaceWith(document.createTextNode("\n"));
    }
    if (!code.firstChild) code.appendChild(document.createElement("br"));
    poserCaret(pre, position);
  }

  /// Defait un bloc de code : chaque ligne redevient un paragraphe. Rend les paragraphes
  /// produits, dont le bouton ¶ a besoin pour reposer la selection.
  function ramenerEnParagraphes(pre: HTMLElement): HTMLParagraphElement[] {
    const lignes = texteDeBloc(pre).replace(/\n+$/, "").split("\n");
    const morceaux = document.createDocumentFragment();
    const produits: HTMLParagraphElement[] = [];
    for (const ligne of lignes) {
      const p = document.createElement("p");
      if (ligne) p.appendChild(document.createTextNode(ligne));
      else p.appendChild(document.createElement("br"));
      morceaux.appendChild(p);
      produits.push(p);
    }
    pre.replaceWith(morceaux);
    const dernier = produits[produits.length - 1];
    if (!dernier) return produits;
    const r = document.createRange();
    r.selectNodeContents(dernier);
    r.collapse(false);
    poserSelection(r);
    return produits;
  }

  /// Place le caret apres `bloc`, dans un paragraphe ou ecrire.
  function sortirDuBloc(bloc: HTMLElement) {
    // La garde a peut-etre deja mis un paragraphe vide juste apres : on s'y place plutot que
    // d'en empiler un second.
    const suivant = bloc.nextElementSibling;
    let cible: HTMLElement;
    if (suivant && suivant.nodeName === "P" && !suivant.textContent) {
      cible = suivant as HTMLElement;
    } else {
      cible = paragrapheVide();
      bloc.parentNode?.insertBefore(cible, bloc.nextSibling);
    }
    const r = document.createRange();
    r.setStart(cible, 0);
    r.collapse(true);
    poserSelection(r);
  }

  /// Entree DANS un bloc de code = un saut de ligne.
  ///
  /// WebKit, lui, CLONE le bloc a chaque Entree : la seule facon d'ecrire « en dessous »
  /// produisait encore du code, et ainsi de suite sans fin.
  function insererSaut(bloc: HTMLElement) {
    const sel = selectionDansEditeur();
    if (!sel) return;
    const r = sel.getRangeAt(0);
    r.deleteContents();
    const saut = document.createTextNode("\n");
    r.insertNode(saut);
    // Un "\n" en toute derniere position n'est pas rendu : sans ce `<br>` la nouvelle ligne
    // resterait invisible et le caret semblerait ne pas avoir bouge.
    const apres = document.createRange();
    apres.selectNodeContents(bloc);
    apres.setStartAfter(saut);
    if (apres.toString().length === 0) {
      saut.parentNode?.insertBefore(document.createElement("br"), saut.nextSibling);
    }
    const place = document.createRange();
    place.setStartAfter(saut);
    place.collapse(true);
    poserSelection(place);
  }

  /// Le caret est-il en fin de bloc, sur une ligne vide ? C'est le geste de sortie : une premiere
  /// Entree ouvre la ligne vide, la suivante quitte le bloc.
  function finDeBlocSurLigneVide(bloc: HTMLElement): boolean {
    const sel = selectionDansEditeur();
    if (!sel) return false;
    const courant = sel.getRangeAt(0);
    const apres = document.createRange();
    apres.selectNodeContents(bloc);
    apres.setStart(courant.endContainer, courant.endOffset);
    if (apres.toString().length > 0) return false;
    const avant = document.createRange();
    avant.selectNodeContents(bloc);
    avant.setEnd(courant.endContainer, courant.endOffset);
    return avant.toString().endsWith("\n");
  }

  /// Retire la ligne vide qu'on vient de quitter, sinon le bloc en garderait une a chaque sortie.
  function retirerLigneVideFinale(bloc: HTMLElement) {
    const contenu = bloc.querySelector("code") ?? bloc;
    // `Range.insertNode` coupe le noeud texte et laisse un morceau VIDE derriere lui : sans
    // l'ignorer, on cherchait le `<br>` de rendu au mauvais endroit et la ligne vide restait.
    let dernier: ChildNode | null = contenu.lastChild;
    while (dernier && dernier.nodeType === Node.TEXT_NODE && dernier.nodeValue === "") {
      const precedent = dernier.previousSibling;
      dernier.remove();
      dernier = precedent;
    }
    if (dernier && dernier.nodeName === "BR") {
      const precedent = dernier.previousSibling;
      dernier.remove();
      dernier = precedent;
    }
    if (dernier?.nodeType === Node.TEXT_NODE && dernier.nodeValue?.endsWith("\n")) {
      (dernier as Text).deleteData((dernier as Text).length - 1, 1);
    }
  }

  /// Entree dans un bloc de code : saut de ligne, ou sortie du bloc.
  ///
  /// Sans ce handler, WebKit clonait le bloc a chaque Entree et aucun geste n'en sortait : sous un
  /// bloc de code en fin de note, plus rien ne pouvait s'ecrire (issue #5).
  function onEditorKeydown(e: KeyboardEvent) {
    if (e.key !== "Enter") return;
    const bloc = blocDeCodeCourant();
    if (!bloc) return;
    e.preventDefault();

    if (e.ctrlKey || e.metaKey) {
      sortirDuBloc(bloc);
    } else if (!texteDeBloc(bloc).trim()) {
      // Bloc de code vide : Entree n'a rien a y garder, elle le retire.
      sortirDuBloc(bloc);
      bloc.remove();
    } else if (finDeBlocSurLigneVide(bloc)) {
      retirerLigneVideFinale(bloc);
      sortirDuBloc(bloc);
    } else {
      insererSaut(bloc);
    }
    onEditorInput();
  }

  function onEditorInput() {
    if (!editorEl) return;
    garantirParagrapheFinal();
    markdownContent = turndown.turndown(editorEl.innerHTML);
    dirty = true;
    scheduleSave();
  }

  function scheduleSave() {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => { flush(); }, 1000);
  }

  /// Un bouton de la barre d'outils n'agit que sur une position de caret dans la note. Sans
  /// caret, `execCommand` ne fait rien du tout : on le DIT au lieu de rester inerte.
  function caretPret(): boolean {
    if (selectionDansEditeur()) return true;
    editorEl?.focus();
    if (selectionDansEditeur()) return true;
    notify(translate("note.placeCaret"), "info", 4000, { report: false });
    return false;
  }

  function format(cmd: string, value: string = "") {
    if (!caretPret()) return;
    document.execCommand(cmd, false, value);
    editorEl?.focus();
    onEditorInput();
  }

  function insertHeading(level: number) {
    if (!caretPret()) return;
    document.execCommand("formatBlock", false, `h${level}`);
    editorEl?.focus();
    onEditorInput();
  }

  /// Bouton `</>` : met le bloc courant en code, ou le defait s'il en est deja un.
  ///
  /// Une BASCULE et non un aller simple : une fois le bloc pose, aucun geste ne le retirait —
  /// Backspace mange son contenu caractere par caractere sans jamais desenvelopper, donc la
  /// seule sortie etait de vider toute la note (issue #5).
  function basculerBlocDeCode() {
    if (!caretPret()) return;
    const bloc = blocDeCodeCourant();
    if (bloc) {
      ramenerEnParagraphes(bloc);
    } else {
      document.execCommand("formatBlock", false, "pre");
      const pre = blocDeCodeCourant();
      if (pre) envelopperDansCode(pre);
    }
    editorEl?.focus();
    onEditorInput();
  }

  /// Blocs de texte touches par la selection, du plus interne, dans l'ordre du document.
  function unitesDeLaSelection(r: Range): HTMLElement[] {
    if (!editorEl) return [];
    const touches = Array.from(editorEl.querySelectorAll<HTMLElement>(BLOCS_DE_TEXTE))
      .filter((el) => r.intersectsNode(el));
    // Un bloc de code se traite d'un seul tenant : le `<code>` qu'il contient n'est pas une
    // unite a part.
    const dehors = touches.filter(
      (el) => !touches.some((autre) => autre !== el && autre.nodeName === "PRE" && autre.contains(el)),
    );
    // Sinon on garde le bloc le plus INTERNE. `marked` rend une citation en
    // `<blockquote><p>` et un element de liste large en `<li><p>` : convertir l'enveloppe
    // imbriquerait deux paragraphes l'un dans l'autre.
    return dehors.filter((el) => !dehors.some((x) => x !== el && el.contains(x)));
  }

  /// Scinde `parent` autour de `enfant` : l'enfant remonte d'un cran, ce qui le suivait
  /// repart dans une copie du parent. C'est ce qui permet de sortir UN element du milieu
  /// d'une liste sans toucher aux autres.
  function scinderAutour(parent: HTMLElement, enfant: HTMLElement) {
    const grand = parent.parentNode;
    if (!grand) return;
    const apres = parent.cloneNode(false) as HTMLElement;
    let n = enfant.nextSibling;
    while (n) {
      const suivant = n.nextSibling;
      apres.appendChild(n);
      n = suivant;
    }
    grand.insertBefore(enfant, parent.nextSibling);
    // Un parent vide de contenu reste peuple des sauts de ligne du rendu Markdown : c'est
    // la presence d'un element ou d'un vrai texte qui dit s'il a encore une raison d'etre.
    if (apres.querySelector("*") || apres.textContent?.trim()) grand.insertBefore(apres, enfant.nextSibling);
    if (!parent.querySelector("*") && !parent.textContent?.trim()) parent.remove();
  }

  /// Ramene un bloc a un paragraphe de premier niveau et rend ce qui a ete produit.
  function remettreAPlat(unite: HTMLElement): HTMLElement[] {
    if (unite.nodeName === "PRE") return ramenerEnParagraphes(unite);
    let bloc = unite;
    if (bloc.nodeName !== "P") {
      const p = document.createElement("p");
      while (bloc.firstChild) p.appendChild(bloc.firstChild);
      bloc.replaceWith(p);
      bloc = p;
    }
    while (bloc.parentElement && bloc.parentElement !== editorEl
      && CONTENEURS_A_SCINDER.includes(bloc.parentElement.nodeName)) {
      scinderAutour(bloc.parentElement, bloc);
    }
    if (!bloc.firstChild) bloc.appendChild(document.createElement("br"));
    return [bloc];
  }

  /// Bouton ¶ : le texte redevient un paragraphe normal.
  ///
  /// Ecrit a la main plutot qu'avec `execCommand("formatBlock", "p")`, dont le banc
  /// WebKitGTK montre les degats : une selection couvrant plusieurs blocs les FUSIONNE en
  /// un seul paragraphe separe par des `<br>` (deux paragraphes perdus a la sauvegarde), une
  /// citation relue du Markdown (`<blockquote><p>`) n'est pas touchee du tout, une liste
  /// devient `<p><ul>…</ul></p>` et un bloc de code ressort en morceaux.
  ///
  /// Ne touche QUE le bloc : gras, italique et liens sont conserves, c'est la mise en forme
  /// de bloc qu'on defait.
  function texteNormal() {
    if (!caretPret()) return;
    const sel = selectionDansEditeur();
    if (!sel) return;
    const r = sel.getRangeAt(0);
    const unites = unitesDeLaSelection(r);
    const replie = r.collapsed;
    const position = replie && unites[0] ? decalageCaret(unites[0]) : 0;
    const produits: HTMLElement[] = [];
    for (const unite of unites) produits.push(...remettreAPlat(unite));
    if (produits.length > 0) {
      if (replie) {
        poserCaret(produits[0], position);
      } else {
        const nouvelle = document.createRange();
        nouvelle.setStartBefore(produits[0]);
        nouvelle.setEndAfter(produits[produits.length - 1]);
        poserSelection(nouvelle);
      }
    }
    editorEl?.focus();
    onEditorInput();
  }
</script>

<svelte:window
  onkeydown={(e) => { if (e.key === "Control" || e.key === "Meta") ctrlEnfonce = true; }}
  onkeyup={(e) => { if (e.key === "Control" || e.key === "Meta") ctrlEnfonce = false; }}
  onblur={() => (ctrlEnfonce = false)}
/>

<div class="editor-panel">
  <div class="editor-header">
    {#if renaming}
      <InlineEdit
        value={file.name}
        onCommit={(v) => { renaming = false; onRename?.(v); }}
        onCancel={() => (renaming = false)}
      />
    {:else}
      <!-- Zone d'edition : elle est deja focalisable et editable au clavier (contenteditable).
       Le clic sert uniquement a ouvrir un lien avec Ctrl, geste souris qui n'a pas
       d'equivalent clavier utile ici. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
      <span class="file-title" ondblclick={() => (renaming = true)}>{file.name}</span>
    {/if}
    <div class="toolbar">
      <button class="tb" onclick={() => format("bold")} title={$trad("note.bold")}><b>G</b></button>
      <button class="tb" onclick={() => format("italic")} title={$trad("note.italic")}><i>I</i></button>
      <button class="tb" onclick={() => format("strikeThrough")} title={$trad("note.strike")}><s>S</s></button>
      <span class="tb-sep"></span>
      <button class="tb" onclick={texteNormal} title={$trad("note.normalText")}>¶</button>
      <button class="tb" onclick={() => insertHeading(1)} title={$trad("note.h1")}>H1</button>
      <button class="tb" onclick={() => insertHeading(2)} title={$trad("note.h2")}>H2</button>
      <button class="tb" onclick={() => insertHeading(3)} title={$trad("note.h3")}>H3</button>
      <span class="tb-sep"></span>
      <button class="tb" onclick={() => format("insertUnorderedList")} title={$trad("note.list")}>•</button>
      <button class="tb" onclick={() => format("insertOrderedList")} title={$trad("note.orderedList")}>1.</button>
      <button class="tb" onclick={() => format("formatBlock", "blockquote")} title={$trad("note.quote")}>❝</button>
      <span class="tb-sep"></span>
      <button class="tb" onclick={basculerBlocDeCode} title={$trad("note.codeBlock")}>&lt;/&gt;</button>
      <button class="tb" onclick={() => { const url = prompt($trad("note.linkUrlPrompt")); if (url) format("createLink", url); }} title={$trad("note.link")}>🔗</button>
    </div>
  </div>

  <!-- Zone d'edition : elle est deja focalisable et editable au clavier (contenteditable).
       Le clic sert uniquement a ouvrir un lien avec Ctrl, geste souris qui n'a pas
       d'equivalent clavier utile ici. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    bind:this={editorEl}
    class="editor"
    class:liens-actifs={ctrlEnfonce}
    contenteditable="true"
    title={$trad("note.openHint")}
    oninput={onEditorInput}
    onkeydown={onEditorKeydown}
    onclick={onEditorClick}
  ></div>
</div>

<style>
  .editor-panel { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  /* Les liens ne deviennent cliquables qu'avec Ctrl : le curseur le montre au survol.
     `:global` est obligatoire — le contenu vient de `innerHTML`, donc Svelte ne voit pas
     ces balises et eliminerait la regle. */
  .editor.liens-actifs :global(a) { cursor: pointer; text-decoration: underline; }
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
