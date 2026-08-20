/**
 * Tests du reperage des adresses dans du texte brut (src/lib/utils/adresses.ts).
 *
 * Lance par `npm run test:front`. Node strip-types execute le module TypeScript directement :
 * pas de dependance de test a installer, et ces cas-la sont ceux qui piegent (ponctuation
 * collee a l'adresse, parenthese apparie, adresse mail nue).
 *
 * Ce fichier vit dans `scripts/` et non dans `src/` a dessein : `tsconfig.json` n'inclut que
 * `src/**`, et `@types/node` n'est pas installe — un test important `node:test` depuis `src/`
 * ferait echouer `npm run check`.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { segmenterLiens, analyserLien } from "../../src/lib/utils/adresses.ts";

/** Les adresses reperees, dans l'ordre. */
function liens(texte) {
  return segmenterLiens(texte)
    .filter((s) => s.kind === "lien")
    .map((s) => [s.texte, s.href]);
}

/** Le texte rendu, tous segments confondus : il doit toujours valoir l'entree. */
function recompose(texte) {
  return segmenterLiens(texte)
    .map((s) => s.texte)
    .join("");
}

test("un texte sans adresse reste un seul morceau de texte", () => {
  const segments = segmenterLiens("Relire la doc API");
  assert.deepEqual(segments, [{ kind: "texte", texte: "Relire la doc API" }]);
});

test("un texte vide ne rend aucun segment", () => {
  assert.deepEqual(segmenterLiens(""), []);
});

test("une adresse http ou https est reperee", () => {
  assert.deepEqual(liens("voir https://exemple.com/a/b?q=1#x"), [
    ["https://exemple.com/a/b?q=1#x", "https://exemple.com/a/b?q=1#x"],
  ]);
  assert.deepEqual(liens("http://localhost:8060/admin"), [
    ["http://localhost:8060/admin", "http://localhost:8060/admin"],
  ]);
});

test("le point final de la phrase n'est pas dans l'adresse", () => {
  assert.deepEqual(liens("va voir https://exemple.com."), [
    ["https://exemple.com", "https://exemple.com"],
  ]);
  assert.equal(recompose("va voir https://exemple.com."), "va voir https://exemple.com.");
});

test("virgule, point-virgule, deux-points, points de suspension et guillemets sont rognes", () => {
  for (const suffixe of [",", ";", ":", "!", "?", "…", "»", "”", "'", '"']) {
    assert.deepEqual(
      liens(`lire https://exemple.com${suffixe} ensuite`),
      [["https://exemple.com", "https://exemple.com"]],
      `suffixe ${suffixe}`,
    );
  }
});

test("une parenthese appariee reste dans l'adresse, une fermante seule est rognee", () => {
  assert.deepEqual(liens("https://fr.wikipedia.org/wiki/Deja_vu_(homonymie)"), [
    [
      "https://fr.wikipedia.org/wiki/Deja_vu_(homonymie)",
      "https://fr.wikipedia.org/wiki/Deja_vu_(homonymie)",
    ],
  ]);
  assert.deepEqual(liens("(voir https://exemple.com/a)"), [
    ["https://exemple.com/a", "https://exemple.com/a"],
  ]);
  assert.deepEqual(liens("cf [https://exemple.com/a]"), [
    ["https://exemple.com/a", "https://exemple.com/a"],
  ]);
});

test("deux adresses a la suite sont separees par leur ponctuation", () => {
  assert.deepEqual(liens("https://a.com, https://b.com."), [
    ["https://a.com", "https://a.com"],
    ["https://b.com", "https://b.com"],
  ]);
  assert.equal(recompose("https://a.com, https://b.com."), "https://a.com, https://b.com.");
});

test("une adresse mail nue devient un mailto", () => {
  assert.deepEqual(liens("ecrire a bob.martin+jira@exemple.co.uk pour la reunion"), [
    ["bob.martin+jira@exemple.co.uk", "mailto:bob.martin+jira@exemple.co.uk"],
  ]);
  assert.deepEqual(liens("ecrire a bob@exemple.com."), [
    ["bob@exemple.com", "mailto:bob@exemple.com"],
  ]);
});

test("une adresse mail DANS une URL ne fait pas un second lien", () => {
  assert.deepEqual(liens("https://exemple.com/u/bob@exemple.com/edit"), [
    ["https://exemple.com/u/bob@exemple.com/edit", "https://exemple.com/u/bob@exemple.com/edit"],
  ]);
});

test("ce qui n'est pas ouvrable tel quel n'est pas souligne", () => {
  // Pas de schema : le backend le refuserait. Souligner un lien mort serait un piege.
  assert.deepEqual(liens("aller sur www.exemple.com"), []);
  assert.deepEqual(liens("ftp://exemple.com/f"), []);
  assert.deepEqual(liens("chemin ../doc/plan.md"), []);
});

test("le texte affiche vaut toujours le texte saisi", () => {
  for (const entree of [
    "Ticket JIRA-42 : https://jira.exemple.com/browse/JIRA-42 (prio 1)",
    "mailto:bob@exemple.com deja ecrit a la main",
    "https://a.com",
    "rien du tout",
    "  espaces   multiples  ",
  ]) {
    assert.equal(recompose(entree), entree, entree);
  }
});

test("analyserLien distingue absolu, incomplet et illisible", () => {
  const absolu = analyserLien("https://exemple.com/a");
  assert.ok(absolu instanceof URL);
  assert.equal(absolu.protocol, "https:");
  assert.equal(analyserLien("www.exemple.com"), "incomplet");
  assert.equal(analyserLien("../doc.md"), "incomplet");
  assert.equal(analyserLien("http://[oups"), "illisible");
});
