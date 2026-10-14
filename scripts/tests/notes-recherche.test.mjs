/**
 * Recherche dans les notes, et coloration de leurs blocs de code.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { plier, occurrences, extrait, chercherDansLesNotes } from "../../src/lib/notes/recherche.ts";
import { devinerLangue, ouvertureDeBloc, segmentsDepuisJetons } from "../../src/lib/notes/coloration.ts";

test("le repli garde la longueur : les positions servent a surligner le texte d'origine", () => {
  const texte = "Résultat ÉTÉ œuvre ß İstanbul";
  assert.equal(plier(texte).length, texte.length);
  assert.equal(plier("Résultat ÉTÉ"), "resultat ete");
});

test("une recherche ignore la casse et les accents", () => {
  assert.deepEqual(occurrences("Le résultat, puis RESULTAT", "resultat"), [3, 18]);
  assert.deepEqual(occurrences("aaaa", "aa"), [0, 2], "pas de chevauchement");
  assert.deepEqual(occurrences("abc", "   "), [], "une requete blanche ne trouve rien");
});

test("l'extrait reste sur la ligne trouvee et coupe ce qui depasse", () => {
  const e = extrait("titre\n" + "x".repeat(60) + " le mot cherché ici " + "y".repeat(60) + "\nsuite", "cherche");
  assert.equal(e.trouve, "cherché");
  assert.ok(e.avant.startsWith("…") && e.apres.endsWith("…"));
  assert.ok(!e.avant.includes("titre") && !e.apres.includes("suite"));
  assert.equal(extrait("rien", "absent"), null);
});

test("un nom trouve passe devant, puis la note qui en parle le plus", () => {
  const notes = [
    { id: 1, name: "divers", content: "docker docker docker" },
    { id: 2, name: "Docker", content: "" },
    { id: 3, name: "autre", content: "docker" },
    { id: 4, name: "vide", content: "rien" },
  ];
  const r = chercherDansLesNotes(notes, "docker");
  assert.deepEqual(r.map((x) => x.note.id), [2, 1, 3]);
  assert.equal(r[1].nombre, 3);
  assert.equal(r[0].extrait, null, "trouvee par son nom seul : pas d'extrait");
});

test("la langue d'un bloc se devine seulement quand c'est sur", () => {
  assert.equal(devinerLangue('{"a": 1, "b": [true]}'), "json");
  assert.equal(devinerLangue('{\n  "a": 1,'), "json", "JSON incomplet pendant la frappe");
  assert.equal(devinerLangue("SELECT * FROM t"), "sql");
  assert.equal(devinerLangue("<?php echo 1;"), "php");
  assert.equal(devinerLangue("<div>a</div>"), "html");
  assert.equal(devinerLangue("<root><a/></root>"), "xml");
  assert.equal(devinerLangue("git status"), "shellscript");
  assert.equal(devinerLangue("services:\n  web:\n    image: nginx\n  - x"), "yaml");
  assert.equal(devinerLangue("Une phrase ordinaire."), null);
  assert.equal(devinerLangue("{ pas du json }"), null);
  assert.equal(devinerLangue(""), null);
});

test("seule une ligne faite d'accents graves ouvre un bloc", () => {
  assert.equal(ouvertureDeBloc("```json"), "json");
  assert.equal(ouvertureDeBloc("  ```  "), "");
  assert.equal(ouvertureDeBloc("```JSON"), "json");
  assert.equal(ouvertureDeBloc("voir ```json"), null);
  assert.equal(ouvertureDeBloc("```json x"), null);
});

test("les segments sont comptes dans le texte entier, sauts de ligne compris", () => {
  const lignes = [
    [{ content: "{", color: "#24292E" }],
    [{ content: "  " , color: "#24292E" }, { content: '"a"', color: "#005CC5" }, { content: ": ", color: "#24292E" }, { content: "1", color: "#005CC5" }],
    [{ content: "}", color: "#24292E" }],
  ];
  const s = segmentsDepuisJetons(lignes, "#24292e");
  const texte = '{\n  "a": 1\n}';
  assert.deepEqual(s.map((x) => texte.slice(x.debut, x.fin)), ['"a"', "1"]);
  assert.ok(s.every((x) => x.couleur === "#005cc5"), "couleur du texte ordinaire non reprise, hexa en minuscules");
});
