/**
 * Ou atterrit un volet qu'on deplace : la geometrie, separee du DOM.
 *
 * **CE QUI SE TROMPE ICI NE SE VOIT PAS EN RELISANT** : le coin d'un volet, ou deux bords
 * sont aussi proches l'un que l'autre ; un voisin qui n'est pas en face et vers lequel le
 * volet partirait sans raison ; une egalite a un pixel pres. Ces cas s'ecrivent avec quatre
 * nombres et s'eprouvent sous node, alors qu'ils demanderaient une souris dans un composant.
 *
 * Le composant ne fait plus que passer des `getBoundingClientRect`.
 */
import type { Cote } from "./disposition";

/** Un rectangle a l'ecran. La forme que rend `getBoundingClientRect`, en plus court. */
export interface Cadre {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** Part du volet, sur chaque bord, qui vaut « pose-le de ce cote ». Au-dela : un echange. */
export const ZONE_BORD = 0.3;

function largeur(r: Cadre): number {
  return r.right - r.left;
}
function hauteur(r: Cadre): number {
  return r.bottom - r.top;
}

/** Le point est-il dans le cadre ? Bords compris : un volet colle a son voisin n'a pas de trou. */
export function dansLeCadre(r: Cadre, x: number, y: number): boolean {
  return x >= r.left && x <= r.right && y >= r.top && y <= r.bottom;
}

/**
 * De quel cote du volet on atterrirait, pour un pointeur en (x, y).
 *
 * **LE BORD LE PLUS PROCHE L'EMPORTE.** Dans un coin, « pres du bord gauche » et « pres du
 * bord haut » sont vrais tous les deux : prendre le premier teste ferait dependre le
 * resultat de l'ordre du code, donc donnerait un geste different selon le coin vise.
 */
export function coteVise(r: Cadre, x: number, y: number, zone = ZONE_BORD): Cote {
  const l = largeur(r);
  const h = hauteur(r);
  // Un volet sans surface (pas encore dispose) : rien de mieux a dire que « au milieu ».
  if (l <= 0 || h <= 0) return "centre";
  const bords: { cote: Cote; part: number }[] = [
    { cote: "gauche", part: (x - r.left) / l },
    { cote: "droite", part: (r.right - x) / l },
    { cote: "haut", part: (y - r.top) / h },
    { cote: "bas", part: (r.bottom - y) / h },
  ];
  const proche = bords.reduce((a, b) => (b.part < a.part ? b : a));
  return proche.part <= zone ? proche.cote : "centre";
}

/**
 * Le volet voisin dans une direction, quand le deplacement vient du MENU et non d'un geste.
 *
 * **LE VOISIN DOIT ETRE EN FACE**, pas seulement « du bon cote » : sans cette condition,
 * « vers le haut » choisirait un volet situe en haut a l'autre bout de l'ecran, et le volet
 * partirait la ou personne ne l'attend. A egalite de distance, le premier de la liste gagne,
 * et la liste suit l'ordre d'affichage (gauche a droite, haut en bas).
 *
 * Rend `null` quand il n'y a personne : l'appelant CACHE alors l'entree de menu au lieu de
 * la laisser sans effet.
 */
export function voisinLePlusProche(
  depart: Cadre,
  candidats: { id: number; cadre: Cadre }[],
  cote: Cote,
  tolerance = 1,
): number | null {
  let meilleur: { id: number; ecart: number } | null = null;
  for (const { id, cadre: r } of candidats) {
    const enFaceX = r.left < depart.right && r.right > depart.left;
    const enFaceY = r.top < depart.bottom && r.bottom > depart.top;
    let ecart: number | null = null;
    if (cote === "gauche" && enFaceY && r.right <= depart.left + tolerance) {
      ecart = depart.left - r.right;
    } else if (cote === "droite" && enFaceY && r.left >= depart.right - tolerance) {
      ecart = r.left - depart.right;
    } else if (cote === "haut" && enFaceX && r.bottom <= depart.top + tolerance) {
      ecart = depart.top - r.bottom;
    } else if (cote === "bas" && enFaceX && r.top >= depart.bottom - tolerance) {
      ecart = r.top - depart.bottom;
    }
    if (ecart === null) continue;
    if (!meilleur || ecart < meilleur.ecart) meilleur = { id, ecart };
  }
  return meilleur?.id ?? null;
}
