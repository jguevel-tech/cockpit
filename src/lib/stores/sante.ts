import { santePage } from "../api/sante";
import { signalerErreur } from "./errors";

// Prouver que la page se PEINT, pas seulement qu'elle tourne.
//
// Un gel du 2026-08-28 s'est produit alors que tout allait bien cote backend : la boucle
// graphique repondait, aucune trace nulle part, et la fenetre etait morte a l'ecran. « Le code
// tourne » et « l'ecran se met a jour » sont deux choses differentes, et rien ne les separait.
//
// La demande d'image du navigateur, elle, les separe : un minuteur continue de tomber quand le
// moteur de rendu a cesse de peindre, une demande d'image NON. On compte donc les images, et on
// le dit au backend, qui ecrit une ligne quand le compte tombe a zero.
const PERIODE = 5000;

let images = 0;

function compter() {
  images += 1;
  requestAnimationFrame(compter);
}

export function surveillerLeRendu() {
  requestAnimationFrame(compter);

  setInterval(() => {
    const compte = images;
    images = 0;
    // Une page cachee ou minimisee ne peint pas, et c'est NORMAL : le signaler accuserait le
    // moteur de rendu a chaque fois que la fenetre passe derriere une autre.
    if (document.visibilityState !== "visible") return;
    santePage(compte).catch((e) => signalerErreur("sante.rendu", String(e)));
  }, PERIODE);
}
