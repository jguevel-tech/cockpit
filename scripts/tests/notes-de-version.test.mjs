/**
 * Tests de la mise en langue des notes de version (src/lib/stores/notesDeVersion.ts).
 *
 * Le risque ici est de trop en faire : ces notes sont du contenu publie, et une reecriture trop
 * large abimerait le texte que l'utilisateur doit lire. Les essais bornent donc autant ce qui
 * est traduit que ce qui ne doit surtout pas l'etre.
 *
 * Le module n'importe qu'un type, donc node l'execute tel quel.
 */
import test from "node:test";
import assert from "node:assert/strict";
import { titresEnLangue, couperLesNotes } from "../../src/lib/stores/notesDeVersion.ts";

/// Faux traducteur : rend la cle, ce qui rend visible CE QUI a ete traduit.
const dire = (cle) => `<${cle}>`;

test("les six sections de Keep a Changelog passent en langue", () => {
  const notes = ["### Added", "### Changed", "### Deprecated", "### Removed", "### Fixed", "### Security"].join("\n");
  assert.equal(
    titresEnLangue(notes, dire),
    [
      "### <changelog.added>",
      "### <changelog.changed>",
      "### <changelog.deprecated>",
      "### <changelog.removed>",
      "### <changelog.fixed>",
      "### <changelog.security>",
    ].join("\n"),
  );
});

test("le niveau du titre est conserve", () => {
  assert.equal(titresEnLangue("## Fixed", dire), "## <changelog.fixed>");
  assert.equal(titresEnLangue("#### fixed", dire), "#### <changelog.fixed>");
});

test("le contenu des notes n'est pas touche", () => {
  const notes = [
    "### Added",
    "",
    "- **Un bouton de compte en haut a droite.** On voit si on est connecte.",
    "- Added support : ce mot au milieu d'une phrase reste tel quel.",
    "",
    "Fixed",
  ].join("\n");
  const rendu = titresEnLangue(notes, dire);
  assert.match(rendu, /^### <changelog\.added>/);
  assert.ok(rendu.includes("- Added support : ce mot au milieu d'une phrase reste tel quel."));
  // Une ligne « Fixed » sans diese n'est pas un titre : c'est du texte.
  assert.ok(rendu.endsWith("\nFixed"));
});

test("le titre « Unreleased » passe en langue, crochets compris", () => {
  assert.equal(titresEnLangue("## [Unreleased]", dire), "## <changelog.unreleased>");
  assert.equal(titresEnLangue("## Unreleased", dire), "## <changelog.unreleased>");
});

test("un titre de VERSION n'est jamais reecrit, malgre ses crochets", () => {
  for (const ligne of ["## [0.47.1] — 2026-08-22", "## [1.0.0]", "## [0.47.1] - 2026-08-22"]) {
    assert.equal(titresEnLangue(ligne, dire), ligne);
  }
});

test("un titre qui n'est pas une section reste tel quel", () => {
  assert.equal(titresEnLangue("### Notes", dire), "### Notes");
  assert.equal(titresEnLangue("### Added later", dire), "### Added later");
});

test("un exemple cite dans un bloc de code reste un exemple", () => {
  const notes = ["### Added", "", "```md", "### Added", "```", ""].join("\n");
  const rendu = titresEnLangue(notes, dire).split("\n");
  assert.equal(rendu[0], "### <changelog.added>");
  assert.equal(rendu[3], "### Added", "le titre dans le bloc de code a ete reecrit");
});

// ── Decoupage : ce qui est rendu tout de suite, et ce qui attend qu'on le demande.

const NOTES = [
  "# Changelog",
  "",
  "Un preambule.",
  "",
  "## [Unreleased]",
  "",
  "## [0.3.0] — 2026-01-03",
  "- trois",
  "",
  "## [0.2.0] — 2026-01-02",
  "- deux",
  "",
  "## [0.1.0] — 2026-01-01",
  "- un",
  "",
].join("\n");

test("le debut garde le preambule et les premieres versions", () => {
  const { tete, reste } = couperLesNotes(NOTES, 2);
  assert.ok(tete.startsWith("# Changelog"), "le preambule doit rester en tete");
  assert.ok(tete.includes("## [Unreleased]"));
  assert.ok(tete.includes("## [0.3.0] — 2026-01-03"));
  assert.ok(!tete.includes("0.2.0"), "0.2.0 devait partir dans le reste");
  assert.ok(reste.startsWith("## [0.2.0]"), "le reste commence a la coupure");
  assert.ok(reste.includes("0.1.0"));
});

test("recolles, les deux morceaux redonnent les notes a l'identique", () => {
  for (const combien of [0, 1, 2, 3, 4, 10]) {
    const { tete, reste } = couperLesNotes(NOTES, combien);
    assert.equal(tete + reste, NOTES, `perte de contenu a ${combien}`);
  }
});

test("rien a couper quand il n'y a pas assez de versions", () => {
  const { tete, reste } = couperLesNotes(NOTES, 99);
  assert.equal(tete, NOTES);
  assert.equal(reste, "");
});

test("un titre de version cite dans le texte ne coupe pas", () => {
  // Les `##` doivent etre en DEBUT de ligne : une mention au fil du texte n'est pas un titre.
  const notes = "# Changelog\n\nvoir ## [0.9.0] plus bas\n\n## [0.9.0]\n- neuf\n";
  const { tete } = couperLesNotes(notes, 1);
  assert.ok(tete.includes("- neuf"), "la seule vraie section a ete coupee trop tot");
});
