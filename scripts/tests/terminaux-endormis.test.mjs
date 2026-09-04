/**
 * Un terminal ENDORMI doit rester visible partout ou l'on liste des terminaux.
 *
 * ## Pourquoi cet essai lit du code source
 *
 * Le defaut qu'il attrape est une OMISSION, pas un calcul : `alive` vaut faux pour tout
 * terminal dont le service n'a plus la session, ce qui est l'etat NORMAL au premier demarrage
 * suivant une extinction du poste. Un `filter(t => t.alive)` a cet endroit ne casse rien, ne
 * leve rien, et vide simplement la liste — l'utilisateur croit avoir perdu ses terminaux alors
 * qu'ils sont tous en base, avec leur ecran.
 *
 * Constate le 2026-09-04 en production : le filtre avait ete retire de l'onglet Terminal et
 * OUBLIE dans deux autres endroits (le magasin de la barre laterale et la fusion des
 * sessions). Un essai de comportement aurait demande de simuler le backend Tauri pour chacun ;
 * lire la source couvre les trois d'un coup, et couvrira le quatrieme qu'on ajoutera.
 *
 * Ce qui reste legitime, et que l'essai autorise donc explicitement : afficher un terminal
 * endormi AUTREMENT (pastille grise, mention « inactif »), et barrer un onglet dont le shell
 * vient de mourir sous les yeux de l'utilisateur.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const RACINE = new URL("../../src/lib", import.meta.url).pathname;

/** Tous les fichiers de code de l'interface, recursivement. */
function fichiers(dossier) {
  return readdirSync(dossier).flatMap((entree) => {
    const chemin = join(dossier, entree);
    if (statSync(chemin).isDirectory()) return fichiers(chemin);
    return /\.(svelte|ts)$/.test(entree) ? [chemin] : [];
  });
}

/**
 * Les formes qui EXCLUENT un terminal endormi d'une liste. On cible le filtrage, jamais la
 * lecture : `class:dead={!t.alive}` et `{#if !t.alive}` decrivent, ils ne cachent pas.
 */
const FILTRES = [
  /\.filter\(\s*\(?\s*\w+\s*\)?\s*=>\s*\w+\.alive\s*\)/,
  /\.filter\(\s*\(?\s*\w+\s*\)?\s*=>\s*\w+\.alive\s*[=!]==?\s*true\s*\)/,
];

test("aucune liste de terminaux ne filtre sur les sessions vivantes", () => {
  const fautifs = [];
  for (const chemin of fichiers(RACINE)) {
    const source = readFileSync(chemin, "utf8");
    source.split("\n").forEach((ligne, index) => {
      if (FILTRES.some((motif) => motif.test(ligne))) {
        fautifs.push(`${chemin.slice(RACINE.length + 1)}:${index + 1} ${ligne.trim()}`);
      }
    });
  }
  assert.deepEqual(
    fautifs,
    [],
    "un terminal endormi est restaurable : le filtrer le fait disparaitre de la liste et " +
      "l'utilisateur croit l'avoir perdu.\n" +
      fautifs.join("\n")
  );
});

/**
 * L'essai ci-dessus ne prouve rien s'il ne voit pas les formes qu'il cherche. On l'eprouve
 * donc sur des lignes fabriquees : sans ce controle, une expression cassee rendrait un vert
 * permanent, exactement comme l'audit de traduction qui a annonce zero avec 42 libelles en dur.
 */
test("les formes de filtrage sont bien reconnues", () => {
  const doivent_tomber = [
    "terminals.set((await listAllTerminals()).filter((t) => t.alive));",
    "const frais = (await listTerminals(name)).filter((t) => t.alive);",
    "const vivants = liste.filter((s) => s.alive === true);",
    "liste.filter(t => t.alive)",
  ];
  for (const ligne of doivent_tomber) {
    assert.ok(
      FILTRES.some((motif) => motif.test(ligne)),
      `forme de filtrage non reconnue : ${ligne}`
    );
  }

  // Et ce qui doit passer : decrire un terminal endormi n'est pas l'exclure.
  const doivent_passer = [
    '{#if !t.alive}<span class="term-state">{$trad("terminals.finished")}</span>{/if}',
    'class:dead={!s.alive}',
    "sessions = existing.map((t) => ({ id: t.id, alive: true, name: t.name }));",
    "if (s) s.alive = false;",
    "const morts = liste.filter((t) => !t.alive);",
  ];
  for (const ligne of doivent_passer) {
    assert.ok(
      !FILTRES.some((motif) => motif.test(ligne)),
      `forme legitime prise pour un filtrage : ${ligne}`
    );
  }
});
