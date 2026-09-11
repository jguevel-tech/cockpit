/**
 * Essais de la visee d'un volet qu'on deplace (src/lib/terminaux/visee.ts).
 *
 * Lance par `npm run test:front`. Ce que ces essais gardent : les coins, ou deux bords sont
 * aussi proches l'un que l'autre, et le voisin « en face » — les deux cas qu'une relecture
 * laisse passer et qu'une souris met dix minutes a verifier.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  coteVise,
  voisinLePlusProche,
  dansLeCadre,
  ZONE_BORD,
} from "../../src/lib/terminaux/visee.ts";

/** Un volet de 200 x 100 pose a l'origine. */
const cadre = { left: 0, right: 200, top: 0, bottom: 100 };

test("le milieu du volet vaut un echange", () => {
  assert.equal(coteVise(cadre, 100, 50), "centre");
});

test("chaque bord est reconnu", () => {
  assert.equal(coteVise(cadre, 5, 50), "gauche");
  assert.equal(coteVise(cadre, 195, 50), "droite");
  assert.equal(coteVise(cadre, 100, 3), "haut");
  assert.equal(coteVise(cadre, 100, 97), "bas");
});

test("dans un coin, le bord le plus proche gagne", () => {
  // Coin haut gauche d'un volet DEUX FOIS plus large que haut : a 10 px des deux bords, on
  // est a 5 % de la largeur mais a 10 % de la hauteur. C'est donc la gauche.
  assert.equal(coteVise(cadre, 10, 10), "gauche");
  // Le meme point sur un volet deux fois plus haut que large donne l'inverse.
  const debout = { left: 0, right: 100, top: 0, bottom: 200 };
  assert.equal(coteVise(debout, 10, 10), "haut");
});

test("la limite de la zone de bord est franche et se regle", () => {
  // Exactement sur la limite : encore un bord, pas le centre.
  assert.equal(coteVise(cadre, 200 * ZONE_BORD, 50), "gauche");
  assert.equal(coteVise(cadre, 200 * ZONE_BORD + 1, 50), "centre");
  // Une zone plus large ramene le meme point sur le bord.
  assert.equal(coteVise(cadre, 200 * ZONE_BORD + 1, 50, 0.45), "gauche");
});

test("un volet sans surface ne fait pas diviser par zero", () => {
  assert.equal(coteVise({ left: 5, right: 5, top: 5, bottom: 5 }, 5, 5), "centre");
});

test("les bords appartiennent au cadre", () => {
  assert.equal(dansLeCadre(cadre, 0, 0), true, "deux volets colles n'ont pas de trou entre eux");
  assert.equal(dansLeCadre(cadre, 200, 100), true);
  assert.equal(dansLeCadre(cadre, 201, 50), false);
  assert.equal(dansLeCadre(cadre, 100, -1), false);
});

// Trois volets : A et B cote a cote en haut, C sur toute la largeur en bas.
const A = { id: 1, cadre: { left: 0, right: 100, top: 0, bottom: 50 } };
const B = { id: 2, cadre: { left: 100, right: 200, top: 0, bottom: 50 } };
const C = { id: 3, cadre: { left: 0, right: 200, top: 50, bottom: 100 } };

test("le voisin se trouve dans la bonne direction", () => {
  assert.equal(voisinLePlusProche(B.cadre, [A, C], "gauche"), 1);
  assert.equal(voisinLePlusProche(A.cadre, [B, C], "droite"), 2);
  assert.equal(voisinLePlusProche(A.cadre, [B, C], "bas"), 3);
  assert.equal(voisinLePlusProche(C.cadre, [A, B], "haut"), 1, "a egalite, le premier affiche");
});

test("il n'y a pas de voisin la ou il n'y a rien", () => {
  assert.equal(voisinLePlusProche(A.cadre, [B, C], "gauche"), null);
  assert.equal(voisinLePlusProche(A.cadre, [B, C], "haut"), null);
  assert.equal(voisinLePlusProche(C.cadre, [A, B], "bas"), null);
  assert.equal(voisinLePlusProche(A.cadre, [], "droite"), null, "seul volet affiche");
});

test("un volet qui n'est pas EN FACE n'est pas un voisin", () => {
  // D est bien a gauche de B, mais posé plus bas : rien ne le touche sur la ligne de B.
  const D = { id: 4, cadre: { left: 0, right: 60, top: 400, bottom: 500 } };
  assert.equal(voisinLePlusProche(B.cadre, [D], "gauche"), null);
  assert.equal(voisinLePlusProche(B.cadre, [D, A], "gauche"), 1, "A, lui, est en face");
});

test("le plus proche gagne quand deux voisins sont en face", () => {
  const loin = { id: 7, cadre: { left: -500, right: -400, top: 0, bottom: 50 } };
  assert.equal(voisinLePlusProche(B.cadre, [loin, A], "gauche"), 1);
  assert.equal(voisinLePlusProche(B.cadre, [A, loin], "gauche"), 1, "l'ordre ne decide pas");
});
