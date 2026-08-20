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
  "cargo test", "api", "timeout", "🔍 timeout", "const TIMEOUT = 30_000;", "const x = 1;",
  "web-1", "Up", "Français", "English",
  "· src/utils/timeout.ts", "→ cockpit-sauvegarde-2026-08-14.db",
  // Noms propres, entites HTML, exemples de commande et noms de signaux : identiques
  // dans les deux langues, ils n'ont rien a faire dans un catalogue.
  "Claude", "&times;", "npm run dev", "SIGTERM", "git push --set-upstream"]);

// Mots identiques dans les deux langues : sigles, noms d'outils, noms propres. Une phrase
// qui n'est faite QUE de ces mots n'a rien a traduire ("Uptime {valeur}", "git push"), mais
// le moindre mot en plus la fait ressortir ("Marketplaces detectes") — c'est ce qui
// distingue cette liste d'un ALLOW pose sur la chaine entiere, toujours a rallonge.
const ALLOW_MOTS = new Set(["px", "ms", "id", "ok", "cpu", "gpu", "rss", "pid", "ram",
  "kernel", "uptime", "swap", "http", "https", "url", "urls", "api", "db", "sql", "json",
  "tmux", "pw-record", "docker", "compose", "git", "push", "pull", "commit", "upstream",
  "npm", "node", "cargo", "rust", "svelte", "claude", "cockpit", "logs", "log",
  "plugin", "plugins", "marketplace", "marketplaces", "agent", "agents",
  "terminal", "terminaux", "terminals", "tag", "tags"]);

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

// Les `${...}` et les `{...}` de Svelte sont du code, pas du texte : un libelle qui n'est
// QUE de l'interpolation (un chemin, un nom de fichier) n'a rien a traduire. En revanche une
// PHRASE qui contient une interpolation reste du texte a traduire — c'est le cas le plus
// courant des libelles rediges, et l'ignorer etait un trou du garde-fou.
const sansCode = (s) => s
  .replace(/\$\{[^}]*\}/g, " ")
  .replace(/\{[^}]*\}/g, " ")
  .replace(/&[a-z]+;/g, " "); // &nbsp; n'est pas le mot « nbsp »

// Mots restants une fois le code retire et les mots identiques dans les deux langues ecartes.
const motsATraduire = (s) => sansCode(s)
  .split(/[^\p{L}-]+/u)
  .filter((mot) => mot.replace(/-/g, "").length > 1 && !ALLOW_MOTS.has(mot.toLowerCase()));

const hasWords = (s) => motsATraduire(s).length > 0 && !estTechnique(s);

// Une chaine tiree d'une EXPRESSION (ternaire, argument, variable) n'est retenue que si
// elle ressemble a une phrase : deux mots au moins. Sans ce filtre, tout ce qui traine
// dans du code — nom de classe CSS, `"success"` passe a notify(), commande shell — serait
// signale, et un garde-fou qui hurle partout finit ignore.
const estPhrase = (s) => hasWords(s) && /\p{L}\s+\p{L}/u.test(sansCode(s));

// Fin du groupe ouvert a `debut` (index de la parenthese/accolade ouvrante), en tenant
// compte des chaines : `$trad("un (deux)")` ne doit pas fermer sur la parenthese du texte.
function finDuGroupe(src, debut) {
  const ouvrant = src[debut];
  const fermant = ouvrant === "(" ? ")" : "}";
  let profondeur = 0;
  let quote = null;
  for (let i = debut; i < src.length; i++) {
    const c = src[i];
    if (quote) {
      if (c === "\\") i++;
      else if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'" || c === "`") quote = c;
    else if (c === ouvrant) profondeur++;
    else if (c === fermant && --profondeur === 0) return i;
  }
  return src.length;
}

// Blanchit les groupes `{...}` : dans le markup, ce qui est entre accolades est du code.
function blanchirAccolades(src) {
  let out = src;
  for (let i = 0; i < src.length; i++) {
    if (src[i] !== "{") continue;
    const fin = finDuGroupe(src, i);
    out = out.slice(0, i) + out.slice(i, fin + 1).replace(/[^\n]/g, " ") + out.slice(fin + 1);
    i = fin;
  }
  return out;
}

// Blanchit `$trad(...)`, `tradN(...)`, `translate(...)` et leurs arguments : ce qui est
// dedans est deja traduit, et une CLE de catalogue ("project.renameHint") ressemble assez
// a du texte pour etre signalee a tort si on la laisse.
function blanchirTraductions(src) {
  const re = /\$?\b(?:trad|tradN|translate|translateN)\s*\(/g;
  let out = src;
  for (const m of [...src.matchAll(re)]) {
    const ouvre = m.index + m[0].length - 1;
    const ferme = finDuGroupe(src, ouvre);
    out = out.slice(0, m.index) +
      out.slice(m.index, ferme + 1).replace(/[^\n]/g, " ") +
      out.slice(ferme + 1);
  }
  return out;
}

// Un libelle affiche ne contient ni option en ligne de commande, ni chemin absolu, ni
// operateur de shell. C'est ce qui separe `Fichier binaire (${taille})` — un libelle a
// parametre — de `docker exec -it ${nom} sh -c '...'`, qui est une commande.
const CODE = /(?:^|\s)(?:-{1,2}[a-zA-Z]|\/[\w.-]+\/|&&|\|\||\$\()/;
const ressembleAuCode = (s) => CODE.test(s);

// Chaines litterales d'un fragment de code (les trois formes de guillemets).
const LITTERAUX = /"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`/g;
const findings = [];
const vus = new Set();

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
  const sansCommentaires = raw
    .replace(/<style[\s\S]*?<\/style>/g, blank)
    .replace(/<!--[\s\S]*?-->/g, blank)
    .replace(/\/\*[\s\S]*?\*\//g, blank)
    // Lignes entierement en commentaire : on ne touche pas aux `//` en fin de ligne de
    // code, qui pourraient etre une URL dans une chaine.
    .replace(/^[ \t]*(\/\/|\*).*$/gm, blank);
  const src = blanchirTraductions(sansCommentaires);

  const push = (index, kind, text, filtre = hasWords) => {
    const t = text.trim();
    if (!t || !filtre(t) || ALLOW.has(t)) return;
    const line = lineOf(src, index);
    // Deux regles voient parfois la meme chaine (l'attribut et l'expression du ternaire
    // qui le remplit) : un signalement par chaine et par ligne suffit.
    const cle = `${rel}:${line}:${t}`;
    if (vus.has(cle)) return;
    vus.add(cle);
    findings.push({ rel, line, kind, text: t.slice(0, 70) });
  };

  // Signale chaque chaine litterale d'un fragment de code (argument d'appel, valeur
  // d'attribut dynamique, initialisation de variable). Filtre `estPhrase` : voir plus haut.
  const pushLitteraux = (debut, fragment, kind) => {
    for (const l of fragment.matchAll(LITTERAUX)) {
      push(debut + l.index, kind, l[0].slice(1, -1), estPhrase);
    }
  };

  // 1) Attributs lisibles. Valeur litterale d'abord — les `{interpolations}` sont
  // tolerees DANS la phrase, seule une valeur qui n'est QUE du code est ignoree.
  for (const attr of TEXT_ATTRS) {
    for (const m of src.matchAll(new RegExp(`\\b${attr}="([^"]*)"`, "g"))) {
      push(m.index, attr, m[1]);
    }
    // Puis la forme `attr={expression}` : un ternaire y cache tres bien deux libelles.
    for (const m of src.matchAll(new RegExp(`\\b${attr}=\\{`, "g"))) {
      const ouvre = m.index + m[0].length - 1;
      const ferme = finDuGroupe(src, ouvre);
      pushLitteraux(ouvre, src.slice(ouvre, ferme + 1), `${attr}=`);
    }
  }

  // 2) Messages passes a l'utilisateur depuis le code. On balaye TOUT l'appel, pas
  // seulement une chaine collee a la parenthese : le message est souvent un ternaire ou
  // une concatenation, et c'est exactement la que les libelles en dur se cachaient.
  for (const m of src.matchAll(/\b(notify|confirm|alert|prompt)\s*\(/g)) {
    const ouvre = m.index + m[0].length - 1;
    pushLitteraux(ouvre, src.slice(ouvre, finDuGroupe(src, ouvre) + 1), m[1]);
  }

  // 3) Proprietes de libelle dans des objets (menus contextuels, onglets...).
  for (const m of src.matchAll(/\b(label|title)\s*:\s*(["'`])((?:(?!\2).)+)\2/g)) {
    push(m.index, `${m[1]}:`, m[3]);
  }

  // 4) Libelle range dans une variable puis affiche par `{maVariable}` : aucune des regles
  // ci-dessus ne le voit, et le detour suffisait a passer sous le radar. C'est le trou le
  // plus productif des trois, d'ou la declaration ET l'affectation (`treeError = "..."`),
  // qui est la forme la plus courante des messages d'erreur d'un composant.
  //
  // Uniquement dans le SCRIPT : dans le markup, `attribut="valeur"` a exactement la meme
  // forme qu'une affectation, et `class="btn btn-small"` serait signale comme un libelle.
  const script = file.endsWith(".svelte")
    ? src.replace(/^[\s\S]*?<script[^>]*>|<\/script>[\s\S]*$/g, blank)
    : src;
  const AFFECTATION = /(?:\b(?:const|let|var)\s+\w+\s*(?::[^=\n]+)?|(?:^|[\s;{}()])[\w.$]+\s*)=\s*(?=["'`])/g;
  for (const m of script.matchAll(AFFECTATION)) {
    const debut = m.index + m[0].length;
    const l = LITTERAUX.exec(script.slice(debut, debut + 400));
    LITTERAUX.lastIndex = 0;
    if (l && l.index === 0 && !ressembleAuCode(l[0])) {
      push(debut, "variable", l[0].slice(1, -1), estPhrase);
    }
  }

  // 5) Texte de markup entre deux balises, hors <script>.
  if (file.endsWith(".svelte")) {
    let markup = src.replace(/<script[\s\S]*?<\/script>/g, blank);
    // Les maquettes de la documentation contiennent du code et des sorties de terminal
    // donnes en exemple (`.d-term`, `.d-code`) : ils s'ecrivent pareil dans toutes les
    // langues. Les LIBELLES d'interface de ces maquettes, eux, sont bien traduits — ils
    // reutilisent les cles de l'interface, ce qui les fait suivre la langue choisie.
    markup = markup.replace(/<div class="d-(term|code)"[\s\S]*?<\/div>/g, blank);
    // Les `{expressions}` sont blanchies AVANT le decoupage : `{icone} Terminaux` est bien
    // un libelle affiche (l'exclure laissait passer toute phrase melee de code), mais une
    // expression contient volontiers un `>` (comparaison, fonction flechee) qui ferait
    // prendre du code pour du texte. Le blanchiment garde les positions, donc le texte
    // signale est relu dans le markup d'origine.
    const decoupe = blanchirAccolades(markup);
    // Les libelles caches dans une expression du markup — `{x ? "oui" : "non"}` — ne sont
    // vus par aucune autre regle : le blanchiment ci-dessus les efface justement.
    for (let i = 0; i < markup.length; i++) {
      if (markup[i] !== "{") continue;
      const fin = finDuGroupe(markup, i);
      pushLitteraux(i, markup.slice(i, fin + 1), "expression");
      i = fin;
    }
    for (const m of decoupe.matchAll(/>([^<>]+)</g)) {
      const debut = m.index + 1;
      if (!hasWords(m[1])) continue;
      push(debut, "markup", markup.slice(debut, debut + m[1].length).replace(/\s+/g, " "));
    }
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
