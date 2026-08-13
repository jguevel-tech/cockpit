#!/usr/bin/env node
/**
 * Prepare une release. Usage : npm run release -- <patch|minor|major>
 *
 * Ce script existe pour une raison precise : c'est TOUJOURS une IA qui fait les releases de
 * ce projet, et une suite d'instructions en prose n'est pas une garantie. Ici les etapes ne
 * peuvent pas etre oubliees parce que ce n'est pas l'IA qui les execute.
 *
 * Il refuse de partir si quoi que ce soit est douteux, et ne pousse JAMAIS :
 * le push est le seul geste humain (regle git du projet).
 */
import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const PKG = resolve(ROOT, "package.json");
const CARGO = resolve(ROOT, "src-tauri/Cargo.toml");
const CHANGELOG = resolve(ROOT, "CHANGELOG.md");

const die = (msg) => {
  console.error(`\n✗ ${msg}\n`);
  process.exit(1);
};
const git = (...args) => execFileSync("git", args, { cwd: ROOT, encoding: "utf8" }).trim();

// --- 1. Verifications prealables ---

const bump = process.argv[2];
if (!["patch", "minor", "major"].includes(bump)) {
  die("Usage : npm run release -- <patch|minor|major>\n\n" +
      "  patch : uniquement des corrections (section Fixed)\n" +
      "  minor : au moins une nouveaute (section Added)\n" +
      "  major : rupture de compatibilite (Removed, ou Changed incompatible)");
}

if (git("status", "--porcelain")) {
  die("L'arbre de travail n'est pas propre. Commite ou remise tes modifications d'abord.");
}

const branch = git("branch", "--show-current");
if (branch !== "main") {
  die(`Tu es sur la branche « ${branch} », les releases se font depuis « main ».`);
}

// --- 2. La section [Unreleased] doit contenir quelque chose ---

const changelog = readFileSync(CHANGELOG, "utf8");
const unreleasedMatch = changelog.match(/^## \[Unreleased\]\s*$([\s\S]*?)(?=^## \[)/m);
if (!unreleasedMatch) {
  die("Section « ## [Unreleased] » introuvable dans CHANGELOG.md.");
}
const notes = unreleasedMatch[1].trim();
if (!notes) {
  die("La section [Unreleased] du CHANGELOG.md est vide.\n\n" +
      "Toute release doit decrire ce qu'elle change : c'est ce texte que les utilisateurs\n" +
      "voient quand la cloche de mise a jour s'allume. Documente les modifications, puis\n" +
      "relance.");
}

// Coherence entre le bump demande et le contenu reel du changelog : un "patch" qui contient
// une section Added est presque toujours une erreur de jugement.
const hasAdded = /^### Added\s*$/m.test(notes);
const hasRemoved = /^### Removed\s*$/m.test(notes);
if (bump === "patch" && hasAdded) {
  die("Le changelog contient une section « Added » mais tu demandes un bump « patch ».\n" +
      "Une nouveaute justifie un « minor ». Corrige le bump ou le changelog.");
}
if (bump !== "major" && hasRemoved) {
  die("Le changelog contient une section « Removed » : c'est une rupture, donc « major ».\n" +
      "Si rien n'est reellement casse, deplace ces lignes sous « Changed ».");
}

// --- 3. Calcul de la nouvelle version ---

const pkg = JSON.parse(readFileSync(PKG, "utf8"));
const [maj, min, pat] = pkg.version.split(".").map(Number);
if ([maj, min, pat].some(Number.isNaN)) {
  die(`Version actuelle illisible dans package.json : « ${pkg.version} »`);
}
const next =
  bump === "major" ? `${maj + 1}.0.0` :
  bump === "minor" ? `${maj}.${min + 1}.0` :
                     `${maj}.${min}.${pat + 1}`;

const today = new Date().toISOString().slice(0, 10);

// --- 4. Ecritures ---

// package.json est la SEULE source de verite de la version : tauri.conf.json la lit
// ("version": "../package.json"). Cargo.toml est aligne pour rester coherent, mais il
// n'est pas lu pour la version de l'app.
pkg.version = next;
writeFileSync(PKG, JSON.stringify(pkg, null, 2) + "\n");

const cargo = readFileSync(CARGO, "utf8");
const cargoOut = cargo.replace(/^version = "[^"]+"$/m, `version = "${next}"`);
if (cargoOut === cargo) die("Impossible de mettre a jour la version dans Cargo.toml.");
writeFileSync(CARGO, cargoOut);

// [Unreleased] devient la section datee, et une [Unreleased] vide est recreee au-dessus.
writeFileSync(
  CHANGELOG,
  changelog.replace(
    /^## \[Unreleased\]\s*$/m,
    `## [Unreleased]\n\n## [${next}] — ${today}`
  )
);

// --- 5. Commit + tag (jamais de push) ---

git("add", "package.json", "src-tauri/Cargo.toml", "CHANGELOG.md");
git("commit", "-m", `Release ${next}`);
git("tag", "-a", `v${next}`, "-m", `Release ${next}\n\n${notes}`);

console.log(`
✓ Release ${pkg.version === next ? next : next} preparee (${bump})

  package.json      ${next}
  Cargo.toml        ${next}
  CHANGELOG.md      section [${next}] — ${today}
  commit            "Release ${next}"
  tag               v${next}

Rien n'a ete pousse. Pour publier :

  git push origin main --follow-tags

GitHub Actions prendra le relais : build de l'AppImage signe, creation de la Release
et publication de latest.json. La cloche s'allumera ensuite chez les utilisateurs.
`);
