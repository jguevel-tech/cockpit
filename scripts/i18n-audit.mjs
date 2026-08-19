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
const ALLOW = new Set(["px", "ms", "fr", "en", "id", "px)", "%", "OK",
  "docker-compose.yml", "docker compose"]);

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

const hasWords = (s) => /\p{L}{2,}/u.test(s);
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
    const markup = src.replace(/<script[\s\S]*?<\/script>/g, (m) => m.replace(/[^\n]/g, " "));
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
