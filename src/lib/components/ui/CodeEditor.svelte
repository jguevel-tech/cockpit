<script lang="ts">
  // Editeur de code leger : textarea transparent superpose au rendu Shiki
  // (technique react-simple-code-editor). Les deux couches partagent police,
  // taille, line-height et padding — l'alignement doit rester EXACT.
  import { highlightCode } from "../../shiki";

  let {
    value = $bindable(""),
    lang,
    dark,
    onSave,
  }: {
    value: string;
    lang: string;
    dark: boolean;
    onSave: () => void;
  } = $props();

  let html = $state("");
  let hlEl: HTMLDivElement | undefined = $state();
  let taEl: HTMLTextAreaElement | undefined = $state();
  let timer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    // Le \n final garde la derniere ligne rendue meme vide (alignement scroll)
    const v = value + "\n";
    const l = lang;
    const d = dark;
    if (timer) clearTimeout(timer);
    timer = setTimeout(async () => {
      html = await highlightCode(v, l, d);
    }, 120);
  });

  $effect(() => {
    taEl?.focus();
  });

  function syncScroll() {
    if (hlEl && taEl) {
      hlEl.scrollTop = taEl.scrollTop;
      hlEl.scrollLeft = taEl.scrollLeft;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      onSave();
    } else if (e.key === "Tab") {
      // execCommand preserve l'historique undo natif du textarea
      e.preventDefault();
      document.execCommand("insertText", false, "  ");
    }
  }
</script>

<div class="editor">
  <div class="hl" bind:this={hlEl} aria-hidden="true">{@html html}</div>
  <textarea
    bind:this={taEl}
    bind:value
    spellcheck="false"
    wrap="off"
    onscroll={syncScroll}
    onkeydown={onKeydown}
  ></textarea>
</div>

<style>
  .editor { position: relative; height: 100%; min-height: 0; }
  /* Les DEUX couches : memes metriques de texte, sinon decalage visuel */
  .hl, textarea {
    font-family: var(--font-mono, monospace);
    font-size: 0.82rem;
    line-height: 1.5;
    tab-size: 4;
    white-space: pre;
    overflow: auto;
  }
  .hl {
    position: absolute; inset: 0;
    pointer-events: none;
    overflow: hidden;
  }
  .hl :global(pre) {
    margin: 0; padding: 0.8rem 1rem;
    background: transparent !important;
    font-family: inherit; font-size: inherit; line-height: inherit;
  }
  .hl :global(code) { font-family: inherit; font-size: inherit; line-height: inherit; }
  textarea {
    position: absolute; inset: 0;
    padding: 0.8rem 1rem;
    background: transparent;
    color: transparent;
    caret-color: var(--text-primary);
    border: none; outline: none; resize: none;
  }
  textarea::selection { background: var(--accent-soft); }
</style>
