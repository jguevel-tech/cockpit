// Highlighter Shiki en bundle fin : uniquement les langages utiles,
// pour ne pas embarquer les ~220 grammaires du bundle complet dans le binaire.
import { createHighlighterCore } from "shiki/core";
import { createJavaScriptRegexEngine } from "shiki/engine/javascript";
import { signalerErreur } from "./stores/errors";

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
    // **LE MOTEUR JAVASCRIPT, PAS CELUI EN WEBASSEMBLY.** Le moteur Oniguruma compile un
    // module WebAssembly, que la politique de securite de la page refuse : tout fichier
    // s'affichait sans couleur. L'ouvrir (`'wasm-unsafe-eval'`) aurait permis a n'importe
    // quel script de la page de compiler du code. Le moteur JavaScript traduit les grammaires
    // en expressions regulieres natives : les 32 langages ci-dessus passent (verifie le
    // 2026-09-28), et il n'y a plus de fichier .wasm a embarquer.
    //
    // **CIBLE ES2024, PAS « auto ».** Le V8 de Chromium 140 sait les modificateurs de regex
    // (`(?i:…)`), donc « auto » les lui confie, et il rate alors les motifs insensibles a la
    // casse : `SELECT` restait sans couleur quand `select` en prenait une (mesure le
    // 2026-10-14 avec le V8 d'Electron). Node 22 ne les sait pas, donc un essai sous node
    // passait. En ES2024, le moteur emule la casse lui-meme et les deux V8 rendent pareil.
    engine: createJavaScriptRegexEngine({ target: "ES2024" }),
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
  } catch (e) {
      signalerErreur("shiki.highlightCode", String(e));
    return `<pre class="shiki"><code>${escapeHtml(code)}</code></pre>`;
  }
}

/** Jetons colores d'un bout de code, ou null si le langage n'est pas embarque. */
export async function jetonsDuCode(
  code: string,
  lang: string,
  dark: boolean,
): Promise<{ lignes: { content: string; color?: string }[][]; couleurParDefaut?: string } | null> {
  const h = await getHighlighter();
  // Un langage inconnu est une saisie ordinaire (« ```truc »), pas une panne : pas de signalement.
  if (!h.getLoadedLanguages().includes(lang)) return null;
  try {
    const r = h.codeToTokens(code, { lang, theme: dark ? "github-dark" : "github-light" });
    return { lignes: r.tokens, couleurParDefaut: r.fg };
  } catch (e) {
    signalerErreur("shiki.jetonsDuCode", String(e));
    return null;
  }
}
