#!/usr/bin/env node
/**
 * Liste ce qui est ARRIVE sur les issues depuis la derniere fois qu'on a regarde.
 * Usage : node scripts/issues-nouveautes.mjs [--marquer]
 *
 * Ce script existe parce que les trois autres facons de savoir « qui attend quoi » ont
 * echoue le 2026-08-20, chacune pour une raison differente :
 *
 *  - les LABELS : ils ne bougent que si quelqu'un les bouge. L'auteur d'une issue ne
 *    posera jamais un label, et nous, on oublie.
 *  - QUI A LE DERNIER MOT : on commente sous le meme compte GitHub que Jimmy
 *    (`jguevel-tech`). Sur ses propres issues, son « c'est tout bon merci » est
 *    indistinguable de notre reponse. C'est exactement ce qui a fait manquer la
 *    confirmation de l'issue #9 pendant une demi-heure.
 *  - LA MEMOIRE DE LA SESSION : elle ne survit pas a la session suivante.
 *
 * Le seul repere qui ne mente pas est donc un repere qu'on tient SOI-MEME : la date du
 * dernier commentaire qu'on a reellement lu, issue par issue. Tout ce qui est plus recent
 * est nouveau, quel que soit son auteur et quels que soient les labels.
 *
 * Le fichier de repere vit hors du depot (`.claude/issues-vues.json`, ignore par git) :
 * le commiter ajouterait un commit a chaque passage, pour une donnee qui ne concerne que
 * la machine qui travaille. Sur une machine neuve, la premiere execution relit tout —
 * c'est bruyant une fois, jamais faux.
 */
import { readFileSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPERE = resolve(ROOT, ".claude/issues-vues.json");
const DEPOT = "jguevel-tech/cockpit";

const marquer = process.argv.includes("--marquer");

const gh = (...args) =>
  execFileSync("gh", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });

let vues = {};
if (existsSync(REPERE)) {
  try {
    vues = JSON.parse(readFileSync(REPERE, "utf8"));
  } catch {
    // Repere illisible : on repart de zero plutot que de planter. Consequence : on relit
    // tout une fois. C'est le bon sens de l'echec pour ce fichier.
    console.error("⚠ repere illisible, on relit tout");
  }
}

const issues = JSON.parse(
  gh("issue", "list", "--repo", DEPOT, "--state", "open", "--limit", "200",
     "--json", "number,title,body,author,comments,labels,createdAt")
);

let nouveautes = 0;
const aMarquer = {};

for (const issue of issues.sort((a, b) => a.number - b.number)) {
  const n = String(issue.number);
  const repere = vues[n] ?? "";
  // Le corps compte comme un evenement : une issue neuve est une nouveaute meme sans
  // commentaire. Sinon les issues #10 a #14, ouvertes puis jamais commentees, seraient
  // restees invisibles.
  const evenements = [
    // Le CORPS, pas le titre : le titre est deja affiche en tete de bloc, et l'afficher a
    // sa place faisait croire qu'une issue decrite etait vide. Bug de ce script, constate
    // sur l'issue #14 dix minutes apres l'avoir ecrit.
    { date: issue.createdAt, qui: issue.author.login,
      texte: issue.body?.trim() || "(pas de description)", genre: "ouverture" },
    ...issue.comments.map((c) => ({
      date: c.createdAt, qui: c.author.login, texte: c.body, genre: "commentaire",
    })),
  ];
  const dernier = evenements[evenements.length - 1].date;
  aMarquer[n] = dernier;

  const neufs = evenements.filter((e) => e.date > repere);
  if (neufs.length === 0) continue;

  nouveautes += neufs.length;
  const labels = issue.labels.map((l) => l.name).join(",") || "aucun label";
  console.log(`\n━━━ #${issue.number} — ${issue.title}`);
  console.log(`    [${labels}]  ouverte par ${issue.author.login}`);
  for (const e of neufs) {
    const extrait = e.texte.replace(/\r?\n/g, " ").slice(0, 700);
    console.log(`  • ${e.date.slice(0, 16)}  ${e.genre} de ${e.qui}`);
    console.log(`    ${extrait}`);
  }
}

if (nouveautes === 0) {
  console.log("Rien de neuf sur les issues depuis la derniere lecture.");
} else {
  console.log(`\n${nouveautes} evenement(s) non lu(s).`);
}

if (marquer) {
  mkdirSync(dirname(REPERE), { recursive: true });
  writeFileSync(REPERE, JSON.stringify(aMarquer, null, 2) + "\n");
  console.log(`\nRepere mis a jour (${Object.keys(aMarquer).length} issues).`);
} else if (nouveautes > 0) {
  console.log("Relancer avec --marquer une fois que tout a ete TRAITE, pas seulement lu.");
}
