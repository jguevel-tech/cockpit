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
  deplacer,
  inserer,
  echanger,
  poserLaSession,
  remplacerFeuille,
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

test("deplacer a gauche met le volet AVANT sa cible", () => {
  // Sans quoi « vers la gauche » et « vers la droite » donneraient le meme resultat.
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "colonnes", 3);
  assert.deepEqual(sessionsAffichees(arbre), [1, 2, 3]);
  const apres = deplacer(arbre, 3, 1, "gauche");
  assert.deepEqual(sessionsAffichees(apres), [3, 1, 2]);
});

test("deplacer a droite met le volet APRES sa cible", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "colonnes", 3);
  assert.deepEqual(sessionsAffichees(deplacer(arbre, 3, 1, "droite")), [1, 3, 2]);
});

test("deplacer en haut ou en bas change le sens de la division", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  const haut = deplacer(arbre, 2, 1, "haut");
  assert.equal(haut.type, "division");
  assert.equal(haut.sens, "lignes", "lache en haut, les volets sont l'un au-dessus de l'autre");
  assert.deepEqual(sessionsAffichees(haut), [2, 1]);
  assert.equal(deplacer(arbre, 2, 1, "bas").sens, "lignes");
  assert.deepEqual(sessionsAffichees(deplacer(arbre, 2, 1, "bas")), [1, 2]);
});

test("lacher au centre echange les deux volets sans changer la forme", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = fixerRatio(arbre, [], 0.3);
  const apres = deplacer(arbre, 2, 1, "centre");
  assert.deepEqual(sessionsAffichees(apres), [2, 1]);
  assert.equal(apres.ratio, 0.3, "un echange ne redimensionne rien");
});

test("un volet ne se deplace jamais sur lui-meme ni sur un volet ferme", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  assert.deepEqual(deplacer(arbre, 1, 1, "droite"), arbre, "sur soi-meme : rien ne bouge");
  assert.deepEqual(deplacer(arbre, 1, 99, "droite"), arbre, "cible disparue : rien ne bouge");
  assert.deepEqual(deplacer(arbre, 99, 1, "droite"), arbre, "source disparue : rien ne bouge");
});

test("deplacer ne duplique ni ne perd un volet", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "lignes", 3);
  arbre = diviser(arbre, 3, "colonnes", 4);
  for (const cote of ["gauche", "droite", "haut", "bas", "centre"]) {
    const apres = deplacer(arbre, 4, 1, cote);
    const vus = sessionsAffichees(apres);
    assert.deepEqual([...vus].sort(), [1, 2, 3, 4], `${cote} : les quatre volets sont toujours la`);
    assert.equal(new Set(vus).size, vus.length, `${cote} : aucun volet en double`);
  }
});

test("deplacer le dernier voisin ne laisse pas une division a un enfant", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  const apres = deplacer(arbre, 2, 1, "bas");
  assert.equal(nombreDeVolets(apres), 2);
  assert.deepEqual(depuisJson(JSON.stringify(apres)), apres, "la disposition reste relisible");
});

test("inserer et echanger ignorent une cible absente", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  assert.deepEqual(inserer(arbre, 99, "lignes", 3, true), arbre);
  assert.deepEqual(echanger(arbre, 99, 98), arbre);
});

// --- La regression de la 0.63.0 : les volets perdus au retour sur l'onglet ---

test("afficher un terminal ne reduit JAMAIS le nombre de volets", () => {
  // Le bug livre : sur une disposition pas encore relue, activer un terminal repartait sur un
  // volet unique, et les volets de l'utilisateur disparaissaient de l'ecran.
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = diviser(arbre, 2, "lignes", 3);
  for (const nouveau of [1, 2, 3, 42]) {
    const apres = poserLaSession(arbre, nouveau, 2);
    assert.equal(nombreDeVolets(apres), 3, `poser ${nouveau} garde les trois volets`);
  }
});

test("un terminal absent prend la place du volet actif, pas d'un autre", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  assert.deepEqual(sessionsAffichees(poserLaSession(arbre, 9, 2)), [1, 9]);
  assert.deepEqual(sessionsAffichees(poserLaSession(arbre, 9, 1)), [9, 2]);
});

test("sans volet actif connu, le premier volet accueille", () => {
  const arbre = diviser(feuille(1), 1, "colonnes", 2);
  assert.deepEqual(sessionsAffichees(poserLaSession(arbre, 9, null)), [9, 2]);
  assert.deepEqual(sessionsAffichees(poserLaSession(arbre, 9, 77)), [9, 2], "actif inconnu");
});

test("un terminal deja affiche ne deplace rien", () => {
  let arbre = diviser(feuille(1), 1, "colonnes", 2);
  arbre = fixerRatio(arbre, [], 0.25);
  assert.deepEqual(poserLaSession(arbre, 2, 1), arbre);
});

test("sans disposition, afficher un terminal en cree une", () => {
  assert.deepEqual(poserLaSession(null, 5, null), feuille(5));
});

test("remplacer une feuille garde la geometrie", () => {
  let arbre = diviser(feuille(1), 1, "lignes", 2);
  arbre = fixerRatio(arbre, [], 0.7);
  const apres = remplacerFeuille(arbre, 2, 8);
  assert.deepEqual(sessionsAffichees(apres), [1, 8]);
  assert.equal(apres.ratio, 0.7);
  assert.equal(apres.sens, "lignes");
});
