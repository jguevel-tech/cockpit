#!/usr/bin/env node
/**
 * Reperage du texte affiche qui n'est pas passe par le catalogue de traduction.
 *
 * Sert de garde-fou au workflow : une fonctionnalite dont les libelles sont ecrits en
 * dur ne serait traduite que dans une langue, et le defaut ne se verrait qu'en basculant
 * l'interface. `npm run i18n:audit` echoue tant qu'il reste du texte non traduit.
 *
 * La completude des catalogues, elle, n'est PAS verifiee ici : `en.ts` doit satisfaire le
 * type derive de `fr.ts`, donc une cle manquante est deja une erreur de `npm run check`.
 * Ce script ne couvre que l'autre moitie du probleme : le texte jamais mis en catalogue.
 */
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const ROOT = new URL("..", import.meta.url).pathname;
const SRC = join(ROOT, "src");

// Fichiers hors perimetre : les catalogues eux-memes, et le point d'entree.
const SKIP_FILES = [/src\/lib\/i18n\//, /src\/main\.ts$/];

// Chaines de markup qui ne sont pas du texte d'interface : unites, symboles, techniques.
// `Ctrl+S` et `Aa` s'ecrivent pareil dans les deux langues ; un raccourci qui contient un
// mot traduisible (Maj / Shift) doit en revanche passer par le catalogue.
const ALLOW = new Set(["px", "ms", "fr", "en", "id", "px)", "%", "OK",
  "docker-compose.yml", "docker compose", "Ctrl+S", "Aa",
  // Sigles et valeurs de configuration, identiques dans les deux langues.
  "CPU", "RSS", "PID", "auto", "in-process", "tmux", "sk-...", "gpt-4o", "ccm-xxx",
  "Ctrl", "Start", "Stop", "Restart", "Pull", "Push", "⬇ Pull", "⬆ Push", "main",
  "⎇ main ▾", "running", "stopped", "running · 8080→80",
  // Donnees d'exemple des maquettes de la documentation : noms de projets fictifs,
  // commandes et termes de recherche. Elles illustrent, elles ne s'affichent pas.
  "Core", "api-gateway", "worker", "mon-projet", "MON-PROJET - 1 ×", "MON-PROJET - 2 ×",
  "COCKPIT - 1", "COCKPIT - 2", "Préprod", "Staging", "Dev", "Tests", "make up",
  "cargo test", "api", "timeout", "🔍 timeout", "const TIMEOUT = 30_000;", "web-1", "Up", "Français", "English",
  "· src/utils/timeout.ts", "→ cockpit-sauvegarde-2026-08-14.db",
  // Noms propres, entites HTML, exemples de commande et noms de signaux : identiques
  // dans les deux langues, ils n'ont rien a faire dans un catalogue.
  "Claude", "&times;", "npm run dev", "SIGTERM"]);

// Attributs dont la valeur est lue par l'utilisateur.
const TEXT_ATTRS = ["title", "placeholder", "aria-label", "alt"];

const files = [];
(function walk(dir) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p);
    else if (/\.(svelte|ts)$/.test(p)) files.push(p);
  }
})(SRC);

// Un identifiant technique n'est pas du texte d'interface : nom de variable
// d'environnement, cle de configuration, chemin, entite HTML, valeur enumeree. Le
// critere est l'absence d'espace jointe a une forme de code — un mot seul comme
// « Chargement… » reste donc signale.
const TECHNIQUE = [
  /^[A-Z0-9_]{3,}$/,           // VARIABLE_ENVIRONNEMENT
  /^[a-z]+([A-Z][a-z0-9]*)+$/, // cleDeConfiguration
  /[/@~]/,                     // chemins, plugin@marketplace
  /^&[a-z]+;$/,                // entites HTML
  /^[\w.-]+\.[a-z]{2,4}$/,     // noms de fichiers : notes.md, logo.png
];
const estTechnique = (s) => !s.includes(" ") && TECHNIQUE.some((re) => re.test(s));

// Les `${...}` sont du code, pas du texte : un libelle qui n'est QUE de l'interpolation
// (un chemin, un nom de fichier) n'a rien a traduire.
const hasWords = (s) =>
  /\p{L}{2,}/u.test(s.replace(/\$\{[^}]*\}/g, "")) && !estTechnique(s);
const findings = [];

function lineOf(text, index) {
  return text.slice(0, index).split("\n").length;
}

for (const file of files.sort()) {
  const rel = relative(ROOT, file);
  if (SKIP_FILES.some((re) => re.test(rel))) continue;
  const raw = readFileSync(file, "utf8");

  // Le CSS et les commentaires ne sont pas du texte d'interface. On les blanchit au lieu
  // de les couper, pour que les numeros de ligne signales restent justes.
  const blank = (m) => m.replace(/[^\n]/g, " ");
  const src = raw
    .replace(/<style[\s\S]*?<\/style>/g, blank)
    .replace(/<!--[\s\S]*?-->/g, blank)
    .replace(/\/\*[\s\S]*?\*\//g, blank)
    // Lignes entierement en commentaire : on ne touche pas aux `//` en fin de ligne de
    // code, qui pourraient etre une URL dans une chaine.
    .replace(/^[ \t]*(\/\/|\*).*$/gm, blank);

  const push = (index, kind, text) => {
    const t = text.trim();
    if (!t || !hasWords(t) || ALLOW.has(t)) return;
    findings.push({ rel, line: lineOf(src, index), kind, text: t.slice(0, 70) });
  };

  // 1) Attributs lisibles, valeur litterale (les valeurs {dynamiques} sont ignorees).
  for (const attr of TEXT_ATTRS) {
    const re = new RegExp(`\\b${attr}="([^"{}]+)"`, "g");
    for (const m of src.matchAll(re)) push(m.index, attr, m[1]);
  }

  // 2) Messages passes a l'utilisateur depuis le code.
  for (const m of src.matchAll(/\b(notify|confirm|alert)\(\s*(["'`])((?:(?!\2).)+)\2/g)) {
    push(m.index, m[1], m[3]);
  }

  // 3) Proprietes de libelle dans des objets (menus contextuels, onglets...).
  for (const m of src.matchAll(/\b(label|title)\s*:\s*(["'`])((?:(?!\2).)+)\2/g)) {
    push(m.index, `${m[1]}:`, m[3]);
  }

  // 4) Texte de markup entre deux balises, hors <script>.
  if (file.endsWith(".svelte")) {
    let markup = src.replace(/<script[\s\S]*?<\/script>/g, blank);
    // Les maquettes de la documentation contiennent du code et des sorties de terminal
    // donnes en exemple (`.d-term`, `.d-code`) : ils s'ecrivent pareil dans toutes les
    // langues. Les LIBELLES d'interface de ces maquettes, eux, sont bien traduits — ils
    // reutilisent les cles de l'interface, ce qui les fait suivre la langue choisie.
    markup = markup.replace(/<div class="d-(term|code)"[\s\S]*?<\/div>/g, blank);
    for (const m of markup.matchAll(/>([^<>{}]+)</g)) push(m.index + 1, "markup", m[1]);
  }
}

const byFile = new Map();
for (const f of findings) byFile.set(f.rel, (byFile.get(f.rel) ?? 0) + 1);

if (process.argv.includes("--summary")) {
  for (const [rel, n] of [...byFile].sort((a, b) => b[1] - a[1])) {
    console.log(String(n).padStart(4), rel);
  }
} else {
  for (const f of findings) console.log(`${f.rel}:${f.line}  [${f.kind}]  ${f.text}`);
}
console.log(`\n${findings.length} chaine(s) non traduite(s) dans ${byFile.size} fichier(s).`);
process.exit(findings.length === 0 ? 0 : 1);
