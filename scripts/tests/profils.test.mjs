/**
 * Essais des profils de fenetre : la regle du nom et ce qu'on en deduit.
 *
 * Ce qu'ils gardent : un profil nomme qui partagerait la base, le socket ou le stockage de
 * la page d'un autre. Rien ne le signalerait : les deux fenetres afficheraient les memes
 * projets, et fermer l'une tuerait les terminaux de l'autre.
 */
import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import profils from "../../coquille/profils.js";
import { REGLE_DU_NOM_DE_PROFIL, nomDeProfilValide } from "../../src/lib/utils/profils.ts";

const VALIDES = ["travail", "a", "perso-2", "0", "a".repeat(32)];
const INVALIDES = ["", "-a", "Perso", "a b", "a/b", "..", "é", "a".repeat(33)];

test("la coquille et l'interface appliquent la meme regle", () => {
  assert.equal(profils.REGLE_DU_NOM.source, REGLE_DU_NOM_DE_PROFIL.source);
  for (const nom of VALIDES) assert.ok(nomDeProfilValide(nom), nom);
  for (const nom of INVALIDES) assert.ok(!nomDeProfilValide(nom), nom);
});

test("un nom invalide est refuse et la regle est citee", () => {
  for (const nom of INVALIDES) assert.throws(() => profils.validerNom(nom), /a-z/);
  for (const nom of VALIDES) assert.equal(profils.validerNom(nom), nom);
});

test("le profil par defaut garde la racine, un profil nomme vit sous profils/", () => {
  assert.equal(profils.dossierDuProfil("/d", null), "/d");
  assert.equal(profils.dossierDuProfil("/d", "travail"), path.join("/d", "profils", "travail"));
  assert.throws(() => profils.dossierDuProfil("/d", "../x"));
});

test("la liste commence par le defaut et ignore ce qui n'est pas un profil", () => {
  const racine = fs.mkdtempSync(path.join(os.tmpdir(), "profils-"));
  try {
    assert.deepEqual(profils.listerProfils(racine), [null], "sans dossier profils/");
    for (const nom of ["travail", "perso", "Majuscule", ".cache"]) {
      fs.mkdirSync(path.join(racine, "profils", nom), { recursive: true });
    }
    fs.writeFileSync(path.join(racine, "profils", "fichier"), "");
    assert.deepEqual(profils.listerProfils(racine), [null, "perso", "travail"]);
  } finally {
    fs.rmSync(racine, { recursive: true, force: true });
  }
});

test("chaque profil nomme a sa partition, le defaut garde la session par defaut", () => {
  assert.equal(profils.partitionDuProfil(null), undefined);
  assert.equal(profils.partitionDuProfil("travail"), "persist:profil-travail");
});

test("le titre nomme le profil sauf pour le defaut", () => {
  assert.equal(profils.titreDuProfil(null), "Cockpit");
  assert.equal(profils.titreDuProfil("travail"), "Cockpit — travail");
});

test("le backend d'un profil nomme ne peut pas retomber sur la base ou le socket d'un autre", () => {
  const base = {
    PATH: "/bin",
    COCKPIT_DB: "/tmp/dev.db",
    COCKPIT_TERMINAUX_SOCKET: "/tmp/t.sock",
    COCKPIT_PROFIL: "herite",
  };
  const nomme = profils.environnementDuBackend(base, "travail");
  assert.equal(nomme.COCKPIT_PROFIL, "travail");
  assert.equal(nomme.COCKPIT_DB, undefined);
  assert.equal(nomme.COCKPIT_TERMINAUX_SOCKET, undefined);
  assert.equal(nomme.PATH, "/bin");

  const defaut = profils.environnementDuBackend(base, null);
  assert.equal(defaut.COCKPIT_PROFIL, undefined, "un profil herite ne doit pas passer");
  assert.equal(defaut.COCKPIT_DB, "/tmp/dev.db", "le defaut garde le reglage de developpement");
  assert.equal(base.COCKPIT_PROFIL, "herite", "la base n'est pas modifiee");
});
