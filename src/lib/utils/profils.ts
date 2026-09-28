/**
 * La regle d'un nom de profil, pour valider la saisie avant de la confier a la coquille.
 *
 * La meme que `coquille/profils.js` (un essai compare les deux) et que
 * `chemins::valider_nom_de_profil` cote backend.
 */
export const REGLE_DU_NOM_DE_PROFIL = /^[a-z0-9][a-z0-9-]{0,31}$/;

export function nomDeProfilValide(nom: string): boolean {
  return REGLE_DU_NOM_DE_PROFIL.test(nom);
}
