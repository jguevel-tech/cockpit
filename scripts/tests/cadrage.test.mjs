/**
 * Tests de la geometrie du cadrage (src/lib/stores/cadrage.ts).
 *
 * Ce qui porte le risque ici est un calcul : une erreur ne plante pas, elle envoie au serveur
 * un morceau d'image qui n'est PAS celui que l'utilisateur a place dans le rond. Rien ne le
 * signalerait, et personne ne saurait dire pourquoi son avatar est de travers.
 *
 * Le module n'a aucune dependance, donc node l'execute tel quel.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  echelleMinimale,
  borner,
  cadrageInitial,
  zoomerAuCentre,
  rectangleSource,
} from "../../src/lib/stores/cadrage.ts";

const SCENE = 300;

test("l'echelle de depart fait remplir la scene par le petit cote", () => {
  // Paysage 600x400 : c'est la HAUTEUR qui doit remplir les 300.
  assert.equal(echelleMinimale(600, 400, SCENE), 0.75);
  // Portrait 400x600 : c'est la LARGEUR.
  assert.equal(echelleMinimale(400, 600, SCENE), 0.75);
  // Carre : l'image tient pile.
  assert.equal(echelleMinimale(400, 400, SCENE), 0.75);
});

test("une image degeneree ne fait pas exploser le calcul", () => {
  assert.equal(echelleMinimale(0, 0, SCENE), 1);
  assert.equal(echelleMinimale(-5, 10, SCENE), 1);
});

test("le cadrage de depart est celui que le serveur aurait choisi : le centre", () => {
  // 600x400 a l'echelle 0.75 fait 450x300 : il faut retirer 75 a gauche pour centrer.
  assert.deepEqual(cadrageInitial(600, 400, SCENE), { x: -75, y: 0 });
  assert.deepEqual(cadrageInitial(400, 600, SCENE), { x: 0, y: -75 });
  assert.deepEqual(cadrageInitial(400, 400, SCENE), { x: 0, y: 0 });
});

test("le rectangle decoupe au depart est bien le carre CENTRAL de l'image", () => {
  const l = 600, h = 400;
  const e = echelleMinimale(l, h, SCENE);
  const r = rectangleSource(cadrageInitial(l, h, SCENE), e, SCENE);
  assert.equal(r.cote, 400, "le cote decoupe doit etre le petit cote de l'image");
  assert.equal(r.sx, 100, "100 px ecartes de chaque cote sur 600");
  assert.equal(r.sy, 0);
  // Et il reste DANS l'image : pas de bord vide.
  assert.ok(r.sx >= 0 && r.sx + r.cote <= l);
  assert.ok(r.sy >= 0 && r.sy + r.cote <= h);
});

test("le decalage est borne : l'image couvre toujours la scene", () => {
  const l = 600, h = 400, e = 0.75; // affichee : 450x300
  // Trop a droite : ramene a 0.
  assert.deepEqual(borner({ x: 50, y: 30 }, l, h, e, SCENE), { x: 0, y: 0 });
  // Trop a gauche : ramene au bord droit collé (300 - 450 = -150).
  assert.deepEqual(borner({ x: -900, y: -900 }, l, h, e, SCENE), { x: -150, y: 0 });
  // Une valeur licite n'est pas touchee.
  assert.deepEqual(borner({ x: -100, y: 0 }, l, h, e, SCENE), { x: -100, y: 0 });
});

test("apres n'importe quel glissement, le decoupage reste dans l'image", () => {
  const l = 600, h = 400, e = echelleMinimale(l, h, SCENE);
  for (const essai of [-1e6, -321, -150, -75, 0, 42, 1e6]) {
    const c = borner({ x: essai, y: essai }, l, h, e, SCENE);
    const r = rectangleSource(c, e, SCENE);
    assert.ok(r.sx >= -1e-9, `sx negatif (${r.sx}) pour x=${essai}`);
    assert.ok(r.sy >= -1e-9, `sy negatif (${r.sy}) pour y=${essai}`);
    assert.ok(r.sx + r.cote <= l + 1e-9, `depasse a droite pour x=${essai}`);
    assert.ok(r.sy + r.cote <= h + 1e-9, `depasse en bas pour y=${essai}`);
  }
});

test("zoomer garde le centre de la scene sur place", () => {
  const centre = SCENE / 2;
  // Un point de l'image sous le centre y reste : on verifie par le rectangle decoupe.
  const l = 400, h = 400, e0 = echelleMinimale(l, h, SCENE);
  const c0 = cadrageInitial(l, h, SCENE);
  const milieuAvant = rectangleSource(c0, e0, SCENE);
  const centreAvant = milieuAvant.sx + milieuAvant.cote / 2;

  const e1 = e0 * 2;
  const c1 = zoomerAuCentre(c0, e0, e1, SCENE);
  const apres = rectangleSource(c1, e1, SCENE);
  const centreApres = apres.sx + apres.cote / 2;

  assert.ok(Math.abs(centreAvant - centreApres) < 1e-9,
    `le centre a bouge : ${centreAvant} -> ${centreApres}`);
  assert.ok(apres.cote < milieuAvant.cote, "zoomer doit decouper PLUS PETIT");
  assert.equal(centre, 150);
});

test("zoomer deux fois decoupe deux fois plus petit", () => {
  const e0 = echelleMinimale(400, 400, SCENE);
  const r1 = rectangleSource({ x: 0, y: 0 }, e0, SCENE);
  const r2 = rectangleSource({ x: 0, y: 0 }, e0 * 2, SCENE);
  assert.equal(r1.cote / r2.cote, 2);
});

test("une echelle absurde ne fait pas bouger le cadre", () => {
  const c = { x: -10, y: -20 };
  assert.deepEqual(zoomerAuCentre(c, 0, 2, SCENE), c);
});
