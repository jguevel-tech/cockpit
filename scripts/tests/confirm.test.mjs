/**
 * Tests de la demande de confirmation (src/lib/stores/confirm.ts).
 *
 * Lance par `npm run test:front`. Ce qui est teste ici est ce qui porte le risque : une
 * promesse qui ne se resoudrait jamais laisserait l'appelant bloque pour toujours, et une
 * seconde demande arrivant sur une premiere non repondue pourrait la laisser en suspens.
 *
 * Le module n'importe que `svelte/store`, donc node l'execute tel quel — voir le commentaire
 * de adresses.test.mjs sur l'emplacement de ces fichiers.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { get } from "svelte/store";
import { demanderConfirmation, confirmation } from "../../src/lib/stores/confirm.ts";

test("valider rend vrai et referme la fenetre", async () => {
  const attente = demanderConfirmation({ message: "Supprimer ?", action: "Supprimer" });
  const demande = get(confirmation);
  assert.ok(demande, "la fenetre doit s'ouvrir");
  assert.equal(demande.message, "Supprimer ?");
  assert.equal(demande.danger, true, "destructif par defaut");

  demande.repondre(true);
  assert.equal(await attente, true);
  assert.equal(get(confirmation), null, "la fenetre doit se refermer");
});

test("annuler rend faux", async () => {
  const attente = demanderConfirmation({ message: "Vraiment ?", action: "Oui" });
  get(confirmation).repondre(false);
  assert.equal(await attente, false);
  assert.equal(get(confirmation), null);
});

test("danger: false pour une action qui ne detruit rien", async () => {
  const attente = demanderConfirmation({ message: "Pousser ?", action: "Push", danger: false });
  const demande = get(confirmation);
  assert.equal(demande.danger, false);
  demande.repondre(true);
  await attente;
});

test("une seconde demande refuse la premiere au lieu de la laisser en suspens", async () => {
  const premiere = demanderConfirmation({ message: "A", action: "A" });
  const seconde = demanderConfirmation({ message: "B", action: "B" });

  // La premiere est resolue a `false` : sans ca, son appelant resterait bloque pour toujours.
  assert.equal(await premiere, false);
  assert.equal(get(confirmation).message, "B", "c'est la seconde qui est affichee");

  get(confirmation).repondre(true);
  assert.equal(await seconde, true);
  assert.equal(get(confirmation), null);
});
