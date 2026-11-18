<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import { marked } from "marked";
  import { creerTurndown, texteDeBloc, versMarkdown } from "../../notes/conversion";
  import { saveNoteFile } from "../../api/storage";
  import { notify } from "../../stores/toast";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import ReadingToggle from "./ReadingToggle.svelte";
  import { toggleReadingMode, readingMode } from "../../stores/ui";
  import type { NoteFile } from "../../types";
  import { trad, translate } from "../../i18n";
  import { ouvrirLien } from "../../utils/liens";
  import { analyserLien, SCHEMAS_OUVRABLES } from "../../utils/adresses";
  import { demanderTexte } from "../../stores/saisie";
  import { themeBase } from "../../stores/appearance";
  import { jetonsDuCode } from "../../shiki";
  import { occurrences } from "../../notes/recherche";
  import { devinerLangue, ouvertureDeBloc, segmentsDepuisJetons } from "../../notes/coloration";

  let {
    file,
    onRename,
    onSaved,
    recherche = "",
  }: {
    file: NoteFile;
    onRename?: (name: string) => void;
    /// Le Markdown vient d'etre enregistre : la liste des notes s'en sert pour chercher dans
    /// ce qu'on vient d'ecrire, sans relire la base.
    onSaved?: (id: number, content: string) => void;
    /// Mot cherche depuis la liste des notes : la note s'ouvre avec ses occurrences surlignees.
    recherche?: string;
  } = $props();

  const turndown = creerTurndown();

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
  /// Panneau entier (en-tete + zone d'edition) : sert a savoir si un Echap vient d'ici.
  let panneauEl: HTMLDivElement | undefined = $state(undefined);
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
      findIdx = 0;
      rafraichirSurlignages();
    });
  }

  /// **UNE NOTE NE S'ENREGISTRE JAMAIS DEPUIS UNE ZONE DE TEXTE ABSENTE.** Constate au banc le
  /// 2026-10-01 : ecrire, puis quitter l'onglet dans la seconde. Le minuteur partait apres la
  /// fermeture, `editorEl` ne designait plus rien, et `innerHTML || ""` enregistrait une note
  /// VIDE par-dessus la vraie. On garde donc l'element (il reste lisible, meme detache) et on
  /// enregistre au moment ou l'editeur se ferme, plutot qu'apres.
  let dernierEditeur: HTMLDivElement | null = null;
  $effect(() => {
    if (editorEl) dernierEditeur = editorEl;
  });
  onDestroy(() => {
    void flush();
  });

  async function flush() {
    if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
    if (!dirty || currentId === null) return;
    const el = editorEl ?? dernierEditeur;
    if (!el) return;
    const id = currentId;
    // Turndown reparcourt tout le DOM : ne pas le faire a chaque frappe dans une note longue.
    const content = versMarkdown(turndown, el.innerHTML, document);
    markdownContent = content;
    dirty = false;
    try {
      await saveNoteFile(id, content);
      onSaved?.(id, content);
    } catch (e) { notify(String(e)); }
  }

  /// Ouvre un lien de la note dans le navigateur du systeme.
  ///
  /// Ctrl+clic et non clic simple : la zone est un editeur, un clic doit pouvoir placer le
  /// curseur pour corriger le texte d'un lien. C'est aussi le geste deja utilise ailleurs
  /// dans Cockpit (terminal, onglet Fichiers, texte d'une tache) et dans les editeurs en
  /// general.
  ///
  /// Le tri de l'adresse et les messages de refus vivent dans utils/liens.ts : le texte
  /// d'une tache ouvre ses liens par le meme chemin.
  async function onEditorClick(e: MouseEvent) {
    if (!(e.ctrlKey || e.metaKey)) return;
    const lien = (e.target as HTMLElement | null)?.closest("a");
    const href = lien?.getAttribute("href");
    if (!href) return;
    e.preventDefault();
    await ouvrirLien(href, "notes.ouvrirLien");
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

  /// Bloc en haut de la zone visible, et sa distance au bord haut.
  ///
  /// Le mode lecture change la LARGEUR du texte, donc ses retours a la ligne : garder
  /// `scrollTop` tel quel ne ramene pas sur le meme paragraphe, et l'ecart grandit avec la
  /// longueur de la note. On repere un bloc et on le remet exactement ou il etait.
  function repereDeLecture(): { bloc: HTMLElement; decalage: number } | null {
    if (!editorEl) return null;
    const hautVue = editorEl.getBoundingClientRect().top;
    for (const bloc of Array.from(editorEl.children) as HTMLElement[]) {
      const r = bloc.getBoundingClientRect();
      if (r.bottom > hautVue) return { bloc, decalage: r.top - hautVue };
    }
    return null;
  }

  function restaurerRepere(repere: { bloc: HTMLElement; decalage: number } | null) {
    if (!repere || !editorEl) return;
    const hautVue = editorEl.getBoundingClientRect().top;
    editorEl.scrollTop += repere.bloc.getBoundingClientRect().top - hautVue - repere.decalage;
  }

  /// Echap quitte le mode lecture, comme il referme le reste de l'application.
  ///
  /// Restreint aux touches frappees DANS le panneau de la note : la palette Ctrl+K et le
  /// panneau de notifications se ferment aussi sur Echap sans arreter la propagation, et
  /// vivent dans <body> (use:portal). Sans cette garde, un seul Echap fermerait la palette ET
  /// deplierait les colonnes.
  /// Ctrl+F dans le panneau de la note : chercher dans la note.
  function onRaccourcis(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || e.shiftKey || e.key.toLowerCase() !== "f") return;
    if (!panneauEl || !(e.target instanceof Node) || !panneauEl.contains(e.target)) return;
    e.preventDefault();
    ouvrirRecherche();
  }

  function onEchap(e: KeyboardEvent) {
    if (e.key !== "Escape" || !$readingMode) return;
    if (!panneauEl || !(e.target instanceof Node) || !panneauEl.contains(e.target)) return;
    basculerLecture();
  }

  /// Bascule le mode lecture SANS faire bouger le sol : meme paragraphe en haut de la vue,
  /// meme curseur de saisie qu'avant si l'utilisateur etait en train d'ecrire.
  async function basculerLecture() {
    const repere = repereDeLecture();
    const sel = selectionDansEditeur();
    const selection = sel ? sel.getRangeAt(0).cloneRange() : null;
    const avaitFocus = document.activeElement === editorEl;

    toggleReadingMode();
    await tick();

    // Le focus d'abord (il peut deplacer le defilement vers le caret), le repere ensuite.
    if (avaitFocus) {
      editorEl?.focus();
      if (selection) poserSelection(selection);
    }
    restaurerRepere(repere);
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
    if (!bloc) {
      if (!e.shiftKey && ouvrirUnBlocTape()) {
        e.preventDefault();
        onEditorInput();
      }
      return;
    }
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
    dirty = true;
    scheduleSave();
    planifierSurlignages();
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

  /// Insere un lien sur la selection.
  ///
  /// Ce bouton ne faisait RIEN sur un Mac : il appelait le `prompt()` du navigateur, absent de
  /// la WebView de macOS. En passant par notre fenetre, un piege apparait : **elle prend le
  /// focus, donc la selection est perdue**. On la CLONE avant d'ouvrir et on la repose avant
  /// d'agir — sinon `createLink` n'a plus rien a habiller. Meme lecon que le repere de lecture.
  async function insererUnLien() {
    if (!caretPret()) return;
    const sel = selectionDansEditeur();
    const portee = sel ? sel.getRangeAt(0).cloneRange() : null;

    const saisi = await demanderTexte({
      message: $trad("note.linkUrlPrompt"),
      action: $trad("note.link"),
      exemple: "https://",
    });
    if (saisi === null) return;

    // La MEME liste blanche que l'ouverture, prise au meme endroit : un lien qu'on refuserait
    // d'ouvrir n'a pas a etre pose. Sinon le clic suivant ne fait rien, et un clic sans effet
    // est vecu comme un bug.
    const cible = analyserLien(saisi);
    if (cible === "incomplet" || cible === "illisible" || !SCHEMAS_OUVRABLES.includes(cible.protocol)) {
      notify($trad("note.linkRefused"), "info", 5000, { report: false });
      return;
    }

    editorEl?.focus();
    if (portee) poserSelection(portee);
    format("createLink", cible.href);
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

  // --- Surlignages : recherche et coloration des blocs de code ---
  //
  // **AUCUN DES DEUX NE TOUCHE AU DOM EDITE.** Ils passent par `CSS.highlights`, qui peint des
  // `Range` sans inserer de balise : le curseur ne bouge pas pendant qu'on tape, et la
  // conversion en Markdown ne voit ni `<mark>` ni `<span>` de couleur. Envelopper le texte,
  // comme le fait la vue d'un fichier, aurait demande de tout defaire avant chaque
  // enregistrement, et deplace le curseur a chaque frappe.

  const BLOCS_SEPARES = new Set(["P", "DIV", "H1", "H2", "H3", "H4", "H5", "H6", "LI", "PRE", "BLOCKQUOTE", "TR", "TD", "TH"]);
  const NOM_RECHERCHE = "note-recherche";
  const NOM_COURANTE = "note-recherche-courante";
  const PREFIXE_COULEUR = "note-c-";

  /// Le texte d'un noeud et, pour chaque noeud texte, sa position dans ce texte. Un `<br>` et
  /// une limite de bloc comptent pour un saut de ligne : un mot cherche ne doit pas se
  /// trouver a cheval sur deux paragraphes, et le code passe a Shiki doit avoir ses lignes.
  interface Carte { texte: string; morceaux: { noeud: Text; debut: number }[] }

  function carteDuTexte(racine: Node): Carte {
    const carte: Carte = { texte: "", morceaux: [] };
    const saut = () => { if (carte.texte && !carte.texte.endsWith("\n")) carte.texte += "\n"; };
    const visiter = (n: Node) => {
      for (const enfant of Array.from(n.childNodes)) {
        if (enfant.nodeType === Node.TEXT_NODE) {
          carte.morceaux.push({ noeud: enfant as Text, debut: carte.texte.length });
          carte.texte += enfant.nodeValue ?? "";
        } else if (enfant.nodeName === "BR") {
          carte.texte += "\n";
        } else {
          const bloc = BLOCS_SEPARES.has(enfant.nodeName);
          if (bloc) saut();
          visiter(enfant);
          if (bloc) saut();
        }
      }
    };
    visiter(racine);
    return carte;
  }

  /// La `Range` qui couvre [debut, fin) dans le texte d'une carte, ou null.
  function plage(carte: Carte, debut: number, fin: number): Range | null {
    const depart = carte.morceaux.find((m) => m.debut <= debut && debut < m.debut + m.noeud.length)
      ?? carte.morceaux.find((m) => m.debut >= debut && m.noeud.length > 0);
    let arrivee: { noeud: Text; debut: number } | undefined;
    for (const m of carte.morceaux) {
      if (m.debut < fin && fin <= m.debut + m.noeud.length) { arrivee = m; break; }
      if (m.debut + m.noeud.length <= fin) arrivee = m;
    }
    if (!depart || !arrivee) return null;
    const r = document.createRange();
    r.setStart(depart.noeud, Math.max(0, debut - depart.debut));
    r.setEnd(arrivee.noeud, Math.min(arrivee.noeud.length, fin - arrivee.debut));
    return r.collapsed ? null : r;
  }

  /// `CSS.highlights` n'existe pas partout (un banc sous un autre moteur) : sans lui, la note
  /// reste lisible et la recherche compte encore ses occurrences.
  const surlignages = typeof CSS !== "undefined" && "highlights" in CSS ? CSS.highlights : null;

  // Recherche dans la note (Ctrl+F)
  let findOpen = $state(false);
  let findQuery = $state("");
  let findIdx = $state(0);
  let findCount = $state(0);
  let findInputEl: HTMLInputElement | undefined = $state(undefined);
  let derniereRecherche = "";

  // La liste des notes cherche un mot : la note s'ouvre dessus.
  $effect(() => {
    const r = recherche.trim();
    untrack(() => {
      if (r) {
        findOpen = true;
        if (findQuery !== r) { findQuery = r; findIdx = 0; }
      } else if (derniereRecherche && findQuery === derniereRecherche) {
        findOpen = false;
        findQuery = "";
      }
      derniereRecherche = r;
      rafraichirRecherche(true);
    });
  });

  function ouvrirRecherche() {
    findOpen = true;
    const sel = selectionDansEditeur();
    const choisi = sel?.toString().trim();
    if (choisi && !choisi.includes("\n")) { findQuery = choisi; findIdx = 0; }
    tick().then(() => { findInputEl?.focus(); findInputEl?.select(); rafraichirRecherche(true); });
  }

  function fermerRecherche() {
    findOpen = false;
    rafraichirRecherche(false);
    editorEl?.focus();
  }

  function allerA(sens: 1 | -1) {
    if (!findCount) return;
    findIdx = ((findIdx + sens) % findCount + findCount) % findCount;
    rafraichirRecherche(true);
  }

  function onFindKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); allerA(e.shiftKey ? -1 : 1); }
    else if (e.key === "Escape") {
      // Sinon l'Echap remonte a la fenetre et quitte aussi le mode lecture.
      e.stopPropagation();
      fermerRecherche();
    }
  }

  function rafraichirRecherche(defiler: boolean) {
    surlignages?.delete(NOM_RECHERCHE);
    surlignages?.delete(NOM_COURANTE);
    if (!editorEl || !findOpen || !findQuery.trim()) { findCount = 0; return; }
    const carte = carteDuTexte(editorEl);
    const longueur = findQuery.trim().length;
    const plages = occurrences(carte.texte, findQuery)
      .map((d) => plage(carte, d, d + longueur))
      .filter((r): r is Range => r !== null);
    findCount = plages.length;
    if (findIdx >= plages.length) findIdx = 0;
    if (!plages.length) return;
    const courante = plages[findIdx];
    if (surlignages) {
      surlignages.set(NOM_RECHERCHE, new Highlight(...plages.filter((r) => r !== courante)));
      surlignages.set(NOM_COURANTE, new Highlight(courante));
    }
    if (defiler) {
      const conteneur = courante.startContainer.parentElement;
      conteneur?.scrollIntoView({ block: "center" });
    }
  }

  // Coloration des blocs de code
  let styleCouleurs: HTMLStyleElement | null = null;
  const couleursDeclarees = new Set<string>();
  let couleursPosees: string[] = [];
  let numeroColoration = 0;

  /// Une regle `::highlight` par couleur, posee une fois. La couleur vient du theme de Shiki,
  /// mais elle entre dans du CSS : on n'accepte qu'un code hexadecimal.
  function declarerCouleur(couleur: string): string | null {
    if (!/^#[0-9a-f]{3,8}$/.test(couleur)) return null;
    const nom = PREFIXE_COULEUR + couleur.slice(1);
    if (!couleursDeclarees.has(nom)) {
      styleCouleurs ??= document.head.appendChild(document.createElement("style"));
      styleCouleurs.append(`::highlight(${nom}) { color: ${couleur}; }\n`);
      couleursDeclarees.add(nom);
    }
    return nom;
  }

  function effacerCouleurs() {
    for (const nom of couleursPosees) surlignages?.delete(nom);
    couleursPosees = [];
  }

  /// La langue d'un bloc : celle qu'il declare, sinon celle qu'on devine.
  function langueDeclaree(pre: HTMLElement): string {
    return pre.querySelector("code")?.className.match(/language-(\S+)/)?.[1] ?? "";
  }

  async function colorier() {
    if (!editorEl || !surlignages) return;
    const numero = ++numeroColoration;
    const sombre = $themeBase === "dark";
    const parCouleur = new Map<string, Range[]>();
    for (const pre of Array.from(editorEl.querySelectorAll<HTMLElement>("pre"))) {
      const carte = carteDuTexte(pre);
      const langue = langueDeclaree(pre) || devinerLangue(carte.texte);
      if (!langue) continue;
      const jetons = await jetonsDuCode(carte.texte, langue, sombre);
      // Une frappe est arrivee pendant le calcul : celui-ci est deja perime.
      if (numero !== numeroColoration) return;
      if (!jetons) continue;
      for (const s of segmentsDepuisJetons(jetons.lignes, jetons.couleurParDefaut)) {
        const nom = declarerCouleur(s.couleur);
        const r = nom ? plage(carte, s.debut, s.fin) : null;
        if (!nom || !r) continue;
        if (!parCouleur.has(nom)) parCouleur.set(nom, []);
        parCouleur.get(nom)!.push(r);
      }
    }
    effacerCouleurs();
    for (const [nom, plages] of parCouleur) {
      surlignages.set(nom, new Highlight(...plages));
      couleursPosees.push(nom);
    }
  }

  function rafraichirSurlignages() {
    rafraichirRecherche(false);
    void colorier();
    suivreLeBlocCourant();
  }

  /// Apres une frappe, on attend une courte pause : recolorier un long bloc a chaque touche
  /// couterait sur le chemin de frappe. Les `Range` posees suivent le texte entre-temps.
  let minuteurSurlignages: ReturnType<typeof setTimeout> | null = null;
  function planifierSurlignages() {
    if (minuteurSurlignages) clearTimeout(minuteurSurlignages);
    minuteurSurlignages = setTimeout(() => { minuteurSurlignages = null; rafraichirSurlignages(); }, 200);
  }

  // Le theme change : les couleurs aussi.
  $effect(() => {
    void $themeBase;
    untrack(() => void colorier());
  });

  onDestroy(() => {
    if (minuteurSurlignages) clearTimeout(minuteurSurlignages);
    effacerCouleurs();
    surlignages?.delete(NOM_RECHERCHE);
    surlignages?.delete(NOM_COURANTE);
    styleCouleurs?.remove();
  });

  // Langue du bloc de code ou se trouve le curseur, pour la barre d'outils
  const LANGUES = ["json", "yaml", "sql", "shellscript", "php", "javascript", "typescript", "html",
    "xml", "css", "twig", "python", "rust", "go", "java", "dockerfile", "ini", "toml", "markdown"];
  let blocCourant: HTMLElement | null = $state(null);
  let langueCourante = $state("");
  let langueDevinee = $state<string | null>(null);

  let corpsEl: HTMLDivElement | undefined = $state(undefined);
  let langueEl: HTMLSelectElement | undefined = $state(undefined);
  let posLangue: { top: number; left: number } | null = $state(null);

  function suivreLeBlocCourant() {
    const bloc = blocDeCodeCourant();
    blocCourant = bloc;
    langueCourante = bloc ? langueDeclaree(bloc) : "";
    langueDevinee = bloc && !langueCourante ? devinerLangue(texteDeBloc(bloc)) : null;
    placerLaLangue();
  }

  /// Coin haut droit du bloc courant, dans le repere de `.corps`. Masquee quand le bloc sort de
  /// la zone visible : elle flotterait sinon au-dessus de l'en-tete.
  function placerLaLangue() {
    if (!blocCourant?.isConnected || !corpsEl || !editorEl) { posLangue = null; return; }
    const b = blocCourant.getBoundingClientRect();
    const c = corpsEl.getBoundingClientRect();
    const e = editorEl.getBoundingClientRect();
    if (b.top < e.top || b.top > e.bottom - 24) { posLangue = null; return; }
    posLangue = { top: b.top - c.top + 4, left: b.right - c.left - 4 };
  }

  /// Le choix vit dans la classe du `<code>`, que la conversion ecrit apres les accents graves
  /// (« ```json ») : il survit donc a l'enregistrement sans rien ajouter.
  function choisirLangue(langue: string) {
    const bloc = blocCourant;
    if (!bloc || !bloc.isConnected) return;
    let code = bloc.querySelector("code");
    if (!code) {
      code = document.createElement("code");
      while (bloc.firstChild) code.appendChild(bloc.firstChild);
      bloc.appendChild(code);
    }
    code.className = langue ? `language-${langue}` : "";
    langueCourante = langue;
    langueDevinee = langue ? null : devinerLangue(texteDeBloc(bloc));
    dirty = true;
    scheduleSave();
    void colorier();
  }

  /// « ```json » puis Entree, comme partout ailleurs : la ligne devient un bloc de code vide
  /// dans cette langue, et le curseur y entre.
  function ouvrirUnBlocTape(): boolean {
    const sel = selectionDansEditeur();
    if (!sel || !editorEl) return false;
    let ligne: Node | null = sel.anchorNode;
    while (ligne && ligne.parentNode !== editorEl) ligne = ligne.parentNode;
    if (!ligne || (ligne.nodeName !== "P" && ligne.nodeName !== "DIV")) return false;
    const langue = ouvertureDeBloc(ligne.textContent ?? "");
    if (langue === null) return false;
    const pre = document.createElement("pre");
    const code = document.createElement("code");
    if (langue) code.className = `language-${langue}`;
    code.appendChild(document.createElement("br"));
    pre.appendChild(code);
    (ligne as HTMLElement).replaceWith(pre);
    const r = document.createRange();
    r.setStart(code, 0);
    r.collapse(true);
    poserSelection(r);
    return true;
  }
</script>

<svelte:window
  onkeydown={(e) => { if (e.key === "Control" || e.key === "Meta") ctrlEnfonce = true; onRaccourcis(e); onEchap(e); }}
  onkeyup={(e) => { if (e.key === "Control" || e.key === "Meta") ctrlEnfonce = false; }}
  onblur={() => (ctrlEnfonce = false)}
  onresize={placerLaLangue}
/>
<svelte:document onselectionchange={() => {
  if (selectionDansEditeur()) suivreLeBlocCourant();
  // Le curseur est parti ailleurs : la liste disparait, sauf si c'est elle qu'on manipule.
  else if (document.activeElement !== langueEl) blocCourant = null;
}} />

<div class="editor-panel" class:lecture={$readingMode} bind:this={panneauEl}>
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
    <!-- Le bouton reste a la MEME place dans les deux etats : c'est ce qui garantit qu'on
         retrouve le chemin du retour la ou on a replie. -->
    <ReadingToggle onToggle={basculerLecture} />
    {#if !findOpen}
      <button class="tb" onclick={ouvrirRecherche} title={$trad("note.find")}>🔍</button>
    {/if}
  </div>
  {#if findOpen}
    <div class="find-bar">
      <input
        bind:this={findInputEl}
        bind:value={findQuery}
        class:no-match={findQuery.trim() && findCount === 0}
        placeholder={$trad("note.findPlaceholder")}
        oninput={() => { findIdx = 0; rafraichirRecherche(true); }}
        onkeydown={onFindKeydown}
      />
      <span class="find-count">{findCount ? `${findIdx + 1}/${findCount}` : "0/0"}</span>
      <button class="tb" onclick={() => allerA(-1)} title={$trad("files.prevMatch")}>↑</button>
      <button class="tb" onclick={() => allerA(1)} title={$trad("files.nextMatch")}>↓</button>
      <button class="tb" onclick={fermerRecherche} title={$trad("files.closeFind")}>×</button>
    </div>
  {/if}

  <!-- Zone d'edition : elle est deja focalisable et editable au clavier (contenteditable).
       Le clic sert uniquement a ouvrir un lien avec Ctrl, geste souris qui n'a pas
       d'equivalent clavier utile ici. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="corps" bind:this={corpsEl}>
  <div
    bind:this={editorEl}
    class="editor"
    class:liens-actifs={ctrlEnfonce}
    contenteditable="true"
    title={$trad("note.openHint")}
    oninput={onEditorInput}
    onkeydown={onEditorKeydown}
    onclick={onEditorClick}
    onscroll={placerLaLangue}
  ></div>
  {#if blocCourant && posLangue}
    <!-- **LA LANGUE SE CHOISIT SUR LE BLOC, PAS DANS LA BARRE.** Posee dans la colonne de
         droite, la liste l'elargissait des que le curseur entrait dans un bloc : la note
         retrecissait et tout son texte se remettait en page sous les yeux. Ici, elle flotte
         au coin du bloc, hors de la zone editee, et rien ne bouge. -->
    <select
      bind:this={langueEl}
      class="langue-bloc"
      style:top="{posLangue.top}px"
      style:left="{posLangue.left}px"
      value={langueCourante}
      title={$trad("note.codeLanguage")}
      onchange={(e) => choisirLangue((e.currentTarget as HTMLSelectElement).value)}
    >
      <option value="">{langueDevinee ? $trad("note.codeLanguageGuessed", { langue: langueDevinee }) : $trad("note.codeLanguageNone")}</option>
      {#each LANGUES as l (l)}<option value={l}>{l}</option>{/each}
      {#if langueCourante && !LANGUES.includes(langueCourante)}<option value={langueCourante}>{langueCourante}</option>{/if}
    </select>
  {/if}
  <!-- **LA BARRE DE MISE EN FORME EST UNE COLONNE A DROITE DE LA NOTE, ET ELLE SUIT LE
       DEFILEMENT.** En haut elle sortait de l'ecran sur une longue note ; posee en bas, elle
       mangeait la feuille. Sur le cote, elle reste a portee sans rien couvrir (demande du
       2026-09-30). -->
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
    <button class="tb" onclick={insererUnLien} title={$trad("note.link")}>🔗</button>
  </div>
  </div>
</div>

<style>
  .editor-panel { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  /* Mode lecture : le compte rendu prend toute la zone, mais la colonne de texte reste
     bornee et CENTREE — c'est la demande (« le texte au milieu tres lisible »), et des
     lignes de 200 caracteres se lisent moins bien que le panneau d'origine. Sur une
     fenetre etroite la borne ne mord pas : le texte occupe simplement tout l'espace. */
  .editor-panel.lecture { max-width: 70rem; margin: 0 auto; width: 100%; }
  /* Les liens ne deviennent cliquables qu'avec Ctrl : le curseur le montre au survol.
     `:global` est obligatoire — le contenu vient de `innerHTML`, donc Svelte ne voit pas
     ces balises et eliminerait la regle. */
  .editor.liens-actifs :global(a) { cursor: pointer; text-decoration: underline; }
  .editor-header { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 0.5rem; flex-wrap: wrap; }
  .file-title { font-weight: 600; font-size: 0.9rem; }

  .corps { position: relative; flex: 1; display: flex; gap: 0.5rem; align-items: stretch; min-height: 0; }
  .corps .editor { flex: 1; min-width: 0; }
  /* Colonne a droite, collee en haut de la vue : une longue note fait defiler la PAGE, et la
     barre suit. Fond OPAQUE (regle des surfaces flottantes). */
  .toolbar {
    display: flex; flex-direction: column; align-items: stretch; gap: 0.2rem;
    position: sticky; top: 0.5rem; align-self: flex-start; z-index: 2;
    background: var(--surface-base); padding: 0.3rem;
    border: 1px solid var(--border-color); border-radius: 6px;
  }
  .tb {
    background: var(--bg-tertiary); border: 1px solid var(--border-color); color: var(--text-secondary);
    padding: 0.2rem 0.45rem; border-radius: 4px; cursor: pointer; font-size: 0.75rem;
    line-height: 1; min-width: 24px; text-align: center;
  }
  .tb:hover { background: var(--accent); color: white; border-color: var(--accent); }
  .langue-bloc {
    position: absolute; transform: translateX(-100%); z-index: 1;
    font-size: 0.7rem; padding: 0.1rem 0.2rem; border-radius: 4px; opacity: 0.85;
    background: var(--surface-base); color: var(--text-secondary); border: 1px solid var(--border-color);
  }
  .langue-bloc:hover, .langue-bloc:focus { opacity: 1; color: var(--text-primary); }
  .find-bar { display: flex; align-items: center; gap: 0.35rem; margin-bottom: 0.5rem; }
  .find-bar input {
    flex: 0 1 16rem; min-width: 6rem; font-size: 0.8rem; padding: 0.2rem 0.45rem;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary);
  }
  .find-bar input.no-match { border-color: var(--error); }
  .find-count { color: var(--text-muted); font-size: 0.72rem; min-width: 3.5ch; text-align: center; }
  .tb-sep { height: 1px; width: 100%; background: var(--border-color); margin: 0.15rem 0; }

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
