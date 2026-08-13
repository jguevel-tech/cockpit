// Highlighter Shiki en bundle fin : uniquement les langages utiles,
// pour ne pas embarquer les ~220 grammaires du bundle complet dans le binaire.
import { createHighlighterCore } from "shiki/core";
import { createOnigurumaEngine } from "shiki/engine/oniguruma";

// Les modules shiki/* sont declares en ambient (vite-env.d.ts), sans types :
// on derive le type du highlighter depuis la factory.
type Highlighter = Awaited<ReturnType<typeof createHighlighterCore>>;

let promise: Promise<Highlighter> | null = null;

function getHighlighter(): Promise<Highlighter> {
  return (promise ??= createHighlighterCore({
    themes: [
      import("@shikijs/themes/github-dark"),
      import("@shikijs/themes/github-light"),
    ],
    langs: [
      import("@shikijs/langs/rust"),
      import("@shikijs/langs/typescript"),
      import("@shikijs/langs/javascript"),
      import("@shikijs/langs/tsx"),
      import("@shikijs/langs/jsx"),
      import("@shikijs/langs/svelte"),
      import("@shikijs/langs/vue"),
      import("@shikijs/langs/python"),
      import("@shikijs/langs/php"),
      import("@shikijs/langs/go"),
      import("@shikijs/langs/ruby"),
      import("@shikijs/langs/java"),
      import("@shikijs/langs/kotlin"),
      import("@shikijs/langs/swift"),
      import("@shikijs/langs/c"),
      import("@shikijs/langs/cpp"),
      import("@shikijs/langs/csharp"),
      import("@shikijs/langs/json"),
      import("@shikijs/langs/yaml"),
      import("@shikijs/langs/toml"),
      import("@shikijs/langs/xml"),
      import("@shikijs/langs/markdown"),
      import("@shikijs/langs/html"),
      import("@shikijs/langs/css"),
      import("@shikijs/langs/scss"),
      import("@shikijs/langs/less"),
      import("@shikijs/langs/shellscript"),
      import("@shikijs/langs/sql"),
      import("@shikijs/langs/dockerfile"),
      import("@shikijs/langs/make"),
      import("@shikijs/langs/ini"),
      import("@shikijs/langs/twig"),
    ],
    engine: createOnigurumaEngine(import("shiki/wasm")),
  }));
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** Rend le code en HTML colore ; fallback <pre> brut si langage inconnu. */
export async function highlightCode(code: string, lang: string, dark: boolean): Promise<string> {
  const theme = dark ? "github-dark" : "github-light";
  try {
    const h = await getHighlighter();
    // Les alias (bash, dockerfile, makefile...) sont resolus par shiki ;
    // langage inconnu -> throw -> fallback <pre> brut.
    return h.codeToHtml(code, { lang, theme });
  } catch {
    return `<pre class="shiki"><code>${escapeHtml(code)}</code></pre>`;
  }
}
