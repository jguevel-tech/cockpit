/**
 * Listes de taches hors projet : « Hors projet », et autant de listes nommees qu'on veut.
 *
 * Une liste n'a pas de table a elle : ses taches portent sa CLE comme projet (`liste:Idees`),
 * ce que la synchronisation transporte deja. Ce qu'une tache ne peut pas porter, l'ordre des
 * listes et une liste encore vide, vit dans un reglage local. Module pur, eprouve sous node.
 * Les deux constantes sont les memes que `todos::SANS_PROJET` et `todos::PREFIXE_LISTE` cote
 * backend, qui refuse tout projet qui les porterait.
 */

export const SANS_PROJET = "";
export const PREFIXE_LISTE = "liste:";
/// Reglage local : les cles des listes, dans l'ordre choisi.
export const CLE_REGLAGE_LISTES = "taches.listes";

export function estUneListe(projet: string): boolean {
  return projet === SANS_PROJET || (projet.startsWith(PREFIXE_LISTE) && projet.length > PREFIXE_LISTE.length);
}

export function cleDeListe(nom: string): string {
  return PREFIXE_LISTE + nom.trim();
}

/// Le nom affiche d'une liste nommee ; `null` pour « Hors projet », dont le libelle est traduit.
export function nomDeListe(cle: string): string | null {
  return cle.startsWith(PREFIXE_LISTE) ? cle.slice(PREFIXE_LISTE.length) : null;
}

/// Le reglage relu. Une valeur abimee ne doit pas faire disparaitre l'ecran : elle vaut « aucune ».
export function lireListes(brut: string | undefined): string[] {
  if (!brut) return [];
  try {
    const v: unknown = JSON.parse(brut);
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string" && x !== SANS_PROJET && estUneListe(x)) : [];
  } catch {
    return []; // reglage illisible : on repart de ce que les taches disent, rien n'est perdu
  }
}

/// Les listes a afficher : celles retenues, dans leur ordre, puis celles que des taches
/// designent sans etre retenues (arrivees d'une autre machine). « Hors projet » vient en tete,
/// et seulement s'il a des taches : sinon il encombrerait l'ecran de quelqu'un qui a nomme ses
/// listes.
export function ordonnerLesListes(retenues: string[], vues: string[]): string[] {
  const sortie: string[] = [];
  if (vues.includes(SANS_PROJET)) sortie.push(SANS_PROJET);
  for (const cle of [...retenues, ...vues]) {
    if (cle !== SANS_PROJET && estUneListe(cle) && !sortie.includes(cle)) sortie.push(cle);
  }
  return sortie;
}
