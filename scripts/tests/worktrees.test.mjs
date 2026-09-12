/**
 * Essais du rattachement des terminaux a leur dossier de travail
 * (src/lib/terminaux/worktrees.ts).
 *
 * Ce que ces essais gardent : un terminal qui DISPARAIT de la liste parce qu'il est ouvert
 * ailleurs, un worktree imbrique dans un autre qui vole ses terminaux au voisin, et deux
 * branches proches qui recoivent la meme couleur — trois choses qu'on ne voit pas en relisant.
 */
import test from "node:test";
import assert from "node:assert/strict";
import {
  worktreeDe,
  grouper,
  teintes,
  libelleDe,
  TEINTES,
} from "../../src/lib/terminaux/worktrees.ts";

const principal = { chemin: "/code/projet", branche: "main", tete: "abc1234", principal: true, verrouille: false, elagable: false };
const ticket = { chemin: "/code/projet.worktrees/ccm-10200", branche: "feat/ccm-10200", tete: "def5678", principal: false, verrouille: false, elagable: false };
const urgent = { chemin: "/code/projet.worktrees/hotfix", branche: "hotfix", tete: "9991111", principal: false, verrouille: false, elagable: false };
const tous = [principal, ticket, urgent];

test("un terminal est rattache au dossier qui le contient", () => {
  assert.equal(worktreeDe("/code/projet", tous), principal.chemin);
  assert.equal(worktreeDe("/code/projet/src/lib", tous), principal.chemin);
  assert.equal(worktreeDe("/code/projet.worktrees/ccm-10200/src", tous), ticket.chemin);
});

test("le dossier le plus precis gagne, pas le premier de la liste", () => {
  // Un worktree DANS un autre : comparer dans l'ordre rattacherait tout au premier.
  const dedans = { chemin: "/code/projet/sous", branche: "sous", tete: "7770000", principal: false, verrouille: false, elagable: false };
  assert.equal(worktreeDe("/code/projet/sous/src", [principal, dedans]), dedans.chemin);
  assert.equal(worktreeDe("/code/projet/sous/src", [dedans, principal]), dedans.chemin, "l'ordre ne decide pas");
});

test("un chemin voisin n'est pas un chemin contenu", () => {
  // `/code/projet-bis` commence par `/code/projet` sans etre dedans.
  const bis = { chemin: "/code/projet-bis", branche: "bis", tete: "1112222", principal: false, verrouille: false, elagable: false };
  assert.equal(worktreeDe("/code/projet-bis/src", [principal, bis]), bis.chemin);
  assert.equal(worktreeDe("/code/projetX", [principal]), null, "voisin, donc pas dedans");
});

test("un separateur final ne change rien", () => {
  const avecBarre = { ...ticket, chemin: "/code/projet.worktrees/ccm-10200/" };
  assert.equal(worktreeDe("/code/projet.worktrees/ccm-10200", [avecBarre]), avecBarre.chemin);
});

test("un terminal ouvert ailleurs n'est pas perdu, il revient au principal", () => {
  const groupes = grouper(tous, [
    { id: 1, cwd: "/code/projet/src" },
    { id: 2, cwd: "/tmp" },
    { id: 3, cwd: null },
    { id: 4, cwd: "/code/projet.worktrees/ccm-10200" },
  ]);
  const parChemin = Object.fromEntries(groupes.map((g) => [g.chemin, g.terminaux]));
  assert.deepEqual(parChemin[principal.chemin], [1, 2, 3], "aucun terminal ne disparait");
  assert.deepEqual(parChemin[ticket.chemin], [4]);
  assert.deepEqual(parChemin[urgent.chemin], [], "un dossier sans terminal reste affiche");
});

test("tous les terminaux se retrouvent dans exactement un groupe", () => {
  const terminaux = [
    { id: 1, cwd: "/code/projet" },
    { id: 2, cwd: "/code/projet.worktrees/hotfix/a/b" },
    { id: 3, cwd: "/ailleurs" },
  ];
  const vus = grouper(tous, terminaux).flatMap((g) => g.terminaux);
  assert.deepEqual([...vus].sort(), [1, 2, 3]);
  assert.equal(new Set(vus).size, vus.length, "aucun terminal compte deux fois");
});

test("sans worktree connu, il n'y a pas de groupe du tout", () => {
  // Un projet qui n'est pas un depot git : la barre ne doit rien afficher, pas un groupe vide.
  assert.deepEqual(grouper([], [{ id: 1, cwd: "/code/projet" }]), []);
});

test("les dossiers affiches ensemble ont tous une couleur differente", () => {
  // Le cas qui arrive vraiment : des branches de tickets qui ne different que par un chiffre.
  // Un hachage du nom y produit des collisions ; une distribution, jamais.
  const proches = ["ccm-10200", "ccm-10201", "ccm-10202", "ccm-01", "ccm-10"].map((n) => ({
    ...ticket,
    chemin: `/code/projet.worktrees/${n}`,
  }));
  const couleurs = [...teintes([principal, ...proches]).values()];
  assert.equal(couleurs.length, 6);
  assert.equal(new Set(couleurs).size, 6, `couleurs en double : ${couleurs.join(", ")}`);
  assert.ok(couleurs.every((c) => TEINTES.includes(c)));
});

test("le principal garde la premiere teinte, quel que soit son chemin", () => {
  const zzz = { ...principal, chemin: "/zzz-tout-a-la-fin" };
  assert.equal(teintes([ticket, zzz]).get(zzz.chemin), TEINTES[0]);
});

test("la couleur d'un dossier ne bouge pas quand on en ajoute un APRES lui", () => {
  const avant = teintes([principal, ticket]);
  const apres = teintes([principal, ticket, urgent]);
  assert.equal(apres.get(ticket.chemin), avant.get(ticket.chemin));
});

test("au-dela de la palette, les couleurs se reprennent sans casser", () => {
  const beaucoup = Array.from({ length: TEINTES.length + 3 }, (_, i) => ({
    ...ticket,
    chemin: `/code/projet.worktrees/b${i}`,
  }));
  const table = teintes(beaucoup);
  assert.equal(table.size, beaucoup.length, "chaque dossier a une couleur");
});

test("une tete detachee s'affiche quand meme", () => {
  assert.equal(libelleDe(ticket), "feat/ccm-10200");
  assert.equal(libelleDe({ ...ticket, branche: null }), "(def5678)");
});
