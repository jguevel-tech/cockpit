/**
 * Listes de taches hors projet : ce qui est une liste, et dans quel ordre elles s'affichent.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { estUneListe, cleDeListe, nomDeListe, lireListes, ordonnerLesListes } from "../../src/lib/todos/listes.ts";

test("un vrai projet n'est jamais pris pour une liste", () => {
  assert.ok(estUneListe(""));
  assert.ok(estUneListe("liste:Idees"));
  assert.ok(!estUneListe("liste:"), "une liste sans nom n'existe pas");
  assert.ok(!estUneListe("cockpit"));
  assert.equal(cleDeListe("  Idees "), "liste:Idees");
  assert.equal(nomDeListe("liste:Idees"), "Idees");
  assert.equal(nomDeListe(""), null);
});

test("un reglage abime vaut aucune liste, sans rien casser", () => {
  assert.deepEqual(lireListes(undefined), []);
  assert.deepEqual(lireListes("pas du json"), []);
  assert.deepEqual(lireListes('{"a":1}'), []);
  assert.deepEqual(lireListes('["liste:A", 3, "projet", "", "liste:B"]'), ["liste:A", "liste:B"]);
});

test("l'ordre retenu d'abord, puis les listes venues d'ailleurs ; Hors projet seulement s'il sert", () => {
  assert.deepEqual(ordonnerLesListes(["liste:B", "liste:A"], ["liste:A", "liste:C"]), ["liste:B", "liste:A", "liste:C"]);
  assert.deepEqual(ordonnerLesListes(["liste:A"], [""]), ["", "liste:A"]);
  assert.deepEqual(ordonnerLesListes(["liste:A"], []), ["liste:A"], "une liste vide retenue reste affichee");
});
