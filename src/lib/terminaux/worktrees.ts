/**
 * A quel dossier de travail appartient un terminal, et de quelle couleur on le montre.
 *
 * **UN SUJET, UNE BRANCHE, UN DOSSIER, UN LOT DE TERMINAUX.** C'est comme ca qu'on travaille
 * vraiment : trois volets ouverts sur un ticket, trois autres sur un correctif urgent, et on
 * passe de l'un a l'autre. Cockpit le permettait deja — chaque terminal garde le dossier ou il
 * a ete ouvert — mais ne le MONTRAIT pas : rien ne disait dans quelle branche on tapait.
 *
 * Ce module ne fait que lire. Il ne cree ni terminal ni worktree, et ne touche pas au disque :
 * il repond a deux questions, « ce terminal appartient a quoi » et « de quelle couleur », et
 * ces deux reponses s'eprouvent sous node.
 */
import type { Worktree } from "../types";

/** Un dossier de travail et les terminaux qui y vivent. */
export interface Groupe {
  /** Chemin du dossier. C'est lui l'identite : deux branches ne partagent pas un dossier. */
  chemin: string;
  /** Ce qu'on affiche : la branche, sinon le nom du dossier. */
  libelle: string;
  principal: boolean;
  /**
   * Le dossier n'existe plus, git en garde seulement la trace. On ne peut rien y ouvrir, et
   * l'interface doit le DIRE plutot que de laisser cliquer dans le vide.
   */
  disparu: boolean;
  /** Identifiants des terminaux qui y sont ouverts, dans l'ordre recu. */
  terminaux: number[];
}

/**
 * Huit teintes, et pas une couleur tiree au hasard.
 *
 * **POURQUOI UNE PALETTE ET PAS UN TOKEN DE THEME.** Il en faut une PAR dossier de travail,
 * donc autant que de branches ouvertes : aucun jeu de tokens ne peut les prevoir. Elles sont
 * donc posees ici, choisies pour rester distinctes entre elles ET lisibles sur les deux
 * themes — elles ne servent qu'a PEINDRE UNE PASTILLE et un liseré, jamais du texte sur un
 * fond, ce qui est justement le cas ou un contraste se retourne d'un theme a l'autre.
 */
export const TEINTES = [
  "#4c8dff",
  "#2dbd8a",
  "#e0a33e",
  "#c96ee8",
  "#ef6f6c",
  "#3fc2c9",
  "#9aa85f",
  "#e8859a",
] as const;

/**
 * Une couleur par dossier de travail, toutes differentes.
 *
 * **LA COULEUR SE DISTRIBUE, ELLE NE SE CALCULE PAS DEPUIS LE NOM.** Premiere version ecrite :
 * une teinte tiree d'un hachage du chemin, stable et sans rien a ranger. L'essai l'a refusee,
 * et il avait raison — huit teintes pour cinq branches de tickets qui ne different que par un
 * chiffre, c'est une collision quasi certaine (le meme calcul que pour deux anniversaires dans
 * une classe). Or la seule propriete qui SERT ici est que deux dossiers affiches cote a cote se
 * distinguent : on distribue donc les teintes dans l'ordre, ce qui la garantit tant qu'il y a
 * moins de dossiers que de couleurs.
 *
 * Le principal prend toujours la premiere, et les autres suivent l'ordre de leur chemin : la
 * couleur d'un dossier ne bouge donc pas tant qu'on n'en cree pas un autre avant lui.
 */
export function teintes(worktrees: Worktree[]): Map<string, string> {
  const ordonnes = [...worktrees].sort((a, b) => {
    if (a.principal !== b.principal) return a.principal ? -1 : 1;
    return a.chemin.localeCompare(b.chemin);
  });
  return new Map(ordonnes.map((w, i) => [w.chemin, TEINTES[i % TEINTES.length]]));
}

/** Le chemin, sans separateur final, pour que deux ecritures du meme dossier se comparent. */
function normaliser(chemin: string): string {
  const sans = chemin.replace(/[\\/]+$/, "");
  return sans.length > 0 ? sans : chemin;
}

/**
 * Ce qu'on affiche pour un dossier de travail.
 *
 * **UN HASH NE DIT RIEN A PERSONNE.** Premiere version : la branche, et a defaut le debut du
 * hash de la tete. Vu chez l'utilisateur, ca donnait deux dossiers nommes `(b897940b)` et
 * `(8b66e238)` — impossible de savoir lequel est lequel. Un worktree a tete detachee garde
 * pourtant un nom de DOSSIER, et ce nom vient de la branche qui l'a cree : c'est lui qu'on
 * montre. Le hash reste, en infobulle, pour situer la tete quand on en a besoin.
 */
export function libelleDe(worktree: Worktree): string {
  if (worktree.branche) return worktree.branche;
  const nom = normaliser(worktree.chemin).split(/[\\/]/).pop();
  return nom && nom.length > 0 ? nom : `(${worktree.tete})`;
}

/**
 * Le dossier de travail auquel appartient un terminal ouvert dans `cwd`.
 *
 * **LE PLUS LONG CHEMIN QUI CONTIENT GAGNE.** Les worktrees vivent a cote du projet
 * (`<projet>.worktrees/<branche>`), mais rien n'interdit qu'un dossier en contienne un autre :
 * comparer dans l'ordre de la liste rattacherait alors au premier venu, donc au hasard.
 *
 * Rend `null` quand le terminal est ailleurs (un `cd` hors du depot a la creation) : l'appelant
 * le rattache au principal plutot que de l'ecarter — **un terminal ne disparait jamais de la
 * liste**, c'est la regle qui a deja coute tous les onglets une fois.
 */
export function worktreeDe(cwd: string | null | undefined, worktrees: Worktree[]): string | null {
  if (!cwd) return null;
  const cible = normaliser(cwd);
  let gagnant: string | null = null;
  for (const w of worktrees) {
    const racine = normaliser(w.chemin);
    const dedans = cible === racine || cible.startsWith(`${racine}/`) || cible.startsWith(`${racine}\\`);
    if (!dedans) continue;
    if (gagnant === null || racine.length > normaliser(gagnant).length) gagnant = w.chemin;
  }
  return gagnant;
}

/**
 * Les dossiers dont git garde la trace alors que le dossier n'existe plus.
 *
 * **ON LES MONTRE, ON NE LES CACHE PAS.** Ils viennent surtout des agents, qui creent leur
 * worktree sous `/tmp` — efface ensuite par le nettoyage du systeme, mais toujours inscrit
 * dans le depot. Les masquer reviendrait a repondre « il n'y a rien » a quelqu'un qui vient
 * justement de voir un agent travailler quelque part. On les AFFICHE donc, marques comme
 * absents, avec de quoi les oublier — c'est la seule facon de comprendre ce que le depot
 * traine.
 */
export function disparus(worktrees: Worktree[]): Worktree[] {
  return worktrees.filter((w) => w.elagable);
}

/**
 * Range les terminaux par dossier de travail.
 *
 * L'ordre des groupes suit celui des worktrees rendus par git (le principal d'abord), et
 * l'ordre des terminaux celui qu'on recoit — c'est celui que l'utilisateur a pose lui-meme.
 *
 * **UN GROUPE VIDE RESTE**, parce qu'un dossier de travail sans terminal ouvert est justement
 * celui sur lequel on veut cliquer pour y entrer.
 */
export function grouper(
  worktrees: Worktree[],
  terminaux: { id: number; cwd?: string | null }[],
): Groupe[] {
  const groupes = worktrees.map((w) => ({
    chemin: w.chemin,
    libelle: libelleDe(w),
    principal: w.principal,
    disparu: w.elagable,
    terminaux: [] as number[],
  }));
  if (groupes.length === 0) return [];
  const parChemin = new Map(groupes.map((g) => [g.chemin, g]));
  const principal = groupes.find((g) => g.principal) ?? groupes[0];
  for (const t of terminaux) {
    const chemin = worktreeDe(t.cwd, worktrees);
    // Un terminal ouvert hors de tout worktree connu revient au principal : le perdre serait
    // pire que le ranger approximativement.
    (parChemin.get(chemin ?? "") ?? principal).terminaux.push(t.id);
  }
  return groupes;
}
