/**
 * La geometrie du cadrage d'une image de profil.
 *
 * Separee du composant parce que ce sont des REGLES : sans aucune dependance, elles s'essaient
 * directement sous node. Le composant ne garde que le geste et le dessin.
 *
 * Le vocabulaire, une fois pour toutes :
 * - la SCENE est le carre visible a l'ecran, de cote `scene` pixels ;
 * - l'ECHELLE dit combien de pixels d'ecran vaut un pixel de l'image ;
 * - le DECALAGE (`x`, `y`) place le coin haut-gauche de l'image dans la scene, donc il est
 *   negatif ou nul des que l'image depasse — ce qui est le cas normal.
 */

/// Un placement de l'image dans la scene.
export interface Cadre {
  x: number;
  y: number;
}

/**
 * L'echelle a laquelle le PETIT cote de l'image remplit exactement la scene.
 *
 * C'est le plancher : en dessous, l'image ne couvrirait plus toute la zone et laisserait un
 * vide dans l'avatar.
 */
export function echelleMinimale(largeur: number, hauteur: number, scene: number): number {
  if (largeur <= 0 || hauteur <= 0) return 1;
  return scene / Math.min(largeur, hauteur);
}

/**
 * Ramene le decalage dans les bornes : l'image doit toujours couvrir la scene.
 *
 * Sans ca, un glissement un peu vif fait apparaitre le fond au bord du rond, et l'avatar sort
 * avec un croissant vide.
 */
export function borner(
  cadre: Cadre,
  largeur: number,
  hauteur: number,
  echelle: number,
  scene: number,
): Cadre {
  return {
    x: Math.min(0, Math.max(scene - largeur * echelle, cadre.x)),
    y: Math.min(0, Math.max(scene - hauteur * echelle, cadre.y)),
  };
}

/**
 * Le cadrage de depart : l'image centree, a l'echelle minimale.
 *
 * C'est EXACTEMENT ce que le serveur ferait tout seul (il recadre au centre). Valider sans
 * rien toucher ne change donc rien au resultat d'avant.
 */
export function cadrageInitial(largeur: number, hauteur: number, scene: number): Cadre {
  const echelle = echelleMinimale(largeur, hauteur, scene);
  return {
    x: (scene - largeur * echelle) / 2,
    y: (scene - hauteur * echelle) / 2,
  };
}

/**
 * Le decalage apres un changement d'echelle, en gardant le CENTRE de la scene sur place.
 *
 * Zoomer autour du coin ferait fuir l'image hors du cadre a chaque cran, et on passerait son
 * temps a la rattraper.
 */
export function zoomerAuCentre(cadre: Cadre, avant: number, apres: number, scene: number): Cadre {
  if (avant <= 0) return cadre;
  const centre = scene / 2;
  return {
    x: centre - ((centre - cadre.x) / avant) * apres,
    y: centre - ((centre - cadre.y) / avant) * apres,
  };
}

/// Le morceau de l'image SOURCE que la scene montre, en pixels de l'image d'origine.
export interface Rectangle {
  sx: number;
  sy: number;
  cote: number;
}

/**
 * Ce qu'il faut decouper dans l'image d'origine pour obtenir ce qu'on voit.
 *
 * C'est la seule formule que le dessin utilise : le canvas recoit ce rectangle et l'etire a la
 * taille de sortie.
 */
export function rectangleSource(cadre: Cadre, echelle: number, scene: number): Rectangle {
  // `-0` est normalise en `0` : la formule le produit des que le decalage vaut zero, et une
  // coordonnee qui s'affiche « -0 » dans un journal fait chercher un signe qui n'existe pas.
  const sansSigne = (v: number) => (v === 0 ? 0 : v);
  return {
    sx: sansSigne(-cadre.x / echelle),
    sy: sansSigne(-cadre.y / echelle),
    cote: scene / echelle,
  };
}
