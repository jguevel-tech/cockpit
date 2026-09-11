/**
 * Essais de l'arbre des volets de terminaux (src/lib/terminaux/disposition.ts).
 *
 * Lance par `npm run test:front`. L'algebre de l'arbre n'a besoin ni du DOM ni d'xterm : ce qui
 * la casse, ce sont les cas de bord — fermer le dernier volet, relire une disposition qui parle
 * de sessions disparues, un ratio venu d'un glissement trop large.
 *
 * Ce fichier vit dans `scripts/` et non dans `src/` a dessein : `tsconfig.json` n'inclut que
 * `src/**`, et un import de `node:test` depuis `src/` ferait echouer `npm run check`.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  feuille,
  diviser,
  retirer,
  fixerRatio,
  nettoyer,
  cheminDe,
  depuisJson,
  sessionsAffichees,
  nombreDeVolets,
  RATIO_MIN,
  RATIO_MAX,
} from "../../src/lib/terminaux/disposition.ts";

test("diviser met le nouveau volet a cote de la cible", () => {
  const apres = diviser(feuille(1), 1, "colonnes", 2);
  assert.equal(apres.type, "division");
  assert.deepEqual(sessionsAffichees(apres), [1, 2]);
  assert.equal(apres.ratio, 0.5, "une division neuve partage a parts egales");
});

test("diviser une cible absente ne touche a rien", () => {
  const avant = diviser(feuille(1), 1, "colonnes", 2);
  const apres = diviser(avant, 99, "lignes", 3);
  assert.deepEqual(sessionsAffichees(apres), [1, 2], "un volet ferme entre-temps ne doit rien casser");
});

test("diviser en profondeur ne touche que la cible", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "lignes", 3);
  assert.deepEqual(sessionsAffichees(arbre), [1, 2, 3]);
  assert.equal(nombreDeVolets(arbre), 3);
});

test("fermer un volet donne toute la place a son voisin", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  const apres = retirer(arbre, 2);
  assert.deepEqual(apres, feuille(1), "le voisin remonte, il ne reste pas une division a un enfant");
});

test("fermer le dernier volet rend rien du tout", () => {
  assert.equal(retirer(feuille(1), 1), null);
  assert.equal(retirer(null, 1), null);
});

test("fermer un volet du milieu garde les autres dans l'ordre", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "colonnes", 3);
  assert.deepEqual(sessionsAffichees(retirer(arbre, 2)), [1, 3]);
});

test("un ratio est borne des deux cotes", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  assert.equal(fixerRatio(arbre, [], 0.001).ratio, RATIO_MIN, "un volet de trois pixels ne se rattrape pas");
  assert.equal(fixerRatio(arbre, [], 12).ratio, RATIO_MAX);
  assert.equal(fixerRatio(arbre, [], 0.35).ratio, 0.35);
});

test("le ratio se pose sur la division visee, pas sur la racine", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "lignes", 3);
  const chemin = cheminDe(arbre, 3);
  assert.ok(chemin, "le volet 3 doit avoir un chemin");
  // Le parent du volet 3 est la division interne : on vise donc son chemin moins le dernier pas.
  const modifie = fixerRatio(arbre, chemin.slice(0, -1), 0.25);
  assert.equal(modifie.ratio, 0.5, "la racine ne doit pas bouger");
  assert.equal(modifie.b.ratio, 0.25);
});

test("relire une disposition oublie les sessions disparues", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "lignes", 3);
  const propre = nettoyer(arbre, [1, 3]);
  assert.deepEqual(sessionsAffichees(propre), [1, 3], "un volet sans session n'affiche rien et ne se ferme pas");
  assert.equal(nettoyer(arbre, []), null, "plus aucune session : plus de disposition");
});

test("une disposition illisible ne casse pas l'onglet", () => {
  assert.equal(depuisJson(null), null);
  assert.equal(depuisJson(""), null);
  assert.equal(depuisJson("{pas du json"), null);
  assert.equal(depuisJson('{"type":"feuille"}'), null, "une feuille sans identifiant ne vaut rien");
  assert.equal(depuisJson('{"type":"division","a":{"type":"feuille","id":1}}'), null, "une division a un seul enfant non plus");
});

test("une disposition relue garde ses volets et borne son ratio", () => {
  const arbre = depuisJson(
    '{"type":"division","sens":"lignes","ratio":40,"a":{"type":"feuille","id":7},"b":{"type":"feuille","id":9}}',
  );
  assert.deepEqual(sessionsAffichees(arbre), [7, 9]);
  assert.equal(arbre.sens, "lignes");
  assert.equal(arbre.ratio, RATIO_MAX, "un ratio aberrant range en base ne doit pas ecraser un volet");
});

test("un aller-retour par le JSON rend la meme disposition", () => {
  let arbre = diviser(feuille(4), 4, "colonnes", 5);
  arbre = diviser(arbre, 5, "lignes", 6);
  arbre = fixerRatio(arbre, [], 0.3);
  assert.deepEqual(depuisJson(JSON.stringify(arbre)), arbre);
});
