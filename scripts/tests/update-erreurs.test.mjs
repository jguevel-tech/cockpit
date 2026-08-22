/**
 * Tests de la lecture d'un echec de mise a jour (src/lib/stores/updateErreurs.ts).
 *
 * Ce qui porte le risque ici n'est pas le code, c'est le SILENCE. Une mise a jour qui echoue
 * en disant seulement « impossible » laisse l'utilisateur sans rien a faire et nous sans rien
 * a lire — c'est arrive une fois, et le diagnostic a demande d'ouvrir un journal sur la
 * machine de quelqu'un.
 *
 * Le module n'importe qu'un type, donc node l'execute tel quel.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { cleErreurMaj, detailErreurMaj } from "../../src/lib/stores/updateErreurs.ts";

test("une release incomplete se dit « pas encore prete » et non « impossible »", () => {
  assert.equal(
    cleErreurMaj("the `platforms` object doesn't contain the target"),
    "update.notReady",
  );
});

test("une panne reseau se dit comme telle", () => {
  for (const brut of [
    "error sending request for url",
    "dns error: failed to lookup",
    "operation timed out",
    "Network is unreachable",
  ]) {
    assert.equal(cleErreurMaj(brut), "update.offline", brut);
  }
});

/**
 * Le cas qui a coute le diagnostic : l'application deplacee depuis son installation. Le texte
 * de l'erreur est TRADUIT par le systeme, donc seul le numero est fiable — un motif ecrit sur
 * la phrase anglaise ne reconnaitrait rien sur une machine en francais.
 */
test("une application deplacee est reconnue quelle que soit la langue du systeme", () => {
  assert.equal(cleErreurMaj("No such file or directory (os error 2)"), "update.moved");
  assert.equal(cleErreurMaj("Aucun fichier ou dossier de ce nom (os error 2)"), "update.moved");
  assert.equal(cleErreurMaj("Datei oder Verzeichnis nicht gefunden (os error 2)"), "update.moved");
});

test("une erreur voisine n'est pas prise pour une application deplacee", () => {
  assert.equal(cleErreurMaj("Permission denied (os error 13)"), null);
  assert.equal(cleErreurMaj("something (os error 21)"), null, "os error 2 ne doit pas matcher 21");
});

test("une panne qu'on ne sait pas nommer garde sa raison visible", () => {
  const brut = "signature verification failed: bad key";
  assert.equal(cleErreurMaj(brut), null);
  assert.equal(detailErreurMaj(brut), brut, "sans ce detail, l'utilisateur n'a rien a rapporter");
});

test("une panne nommee n'affiche pas de trace technique en plus", () => {
  assert.equal(
    detailErreurMaj("Aucun fichier ou dossier de ce nom (os error 2)"),
    null,
    "un message clair suivi d'une trace fait douter du message",
  );
});

test("un detail tres long est borne", () => {
  assert.equal(detailErreurMaj("x".repeat(1000)).length, 300);
});
