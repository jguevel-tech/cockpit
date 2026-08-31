import { santePage } from "../api/sante";
import { signalerErreur } from "./errors";

// Prouver que la page se PEINT, pas seulement qu'elle tourne.
//
// Un gel du 2026-08-28 s'est produit alors que tout allait bien cote backend : la boucle
// graphique repondait, aucune trace nulle part, et la fenetre etait morte a l'ecran. « Le code
// tourne » et « l'ecran se met a jour » sont deux choses differentes, et rien ne les separait.
//
// La demande d'image du navigateur, elle, les separe : un minuteur continue de tomber quand le
// moteur de rendu a cesse de peindre, une demande d'image NON.
//
// **ON PARLE MEME QUAND LA FENETRE EST CACHEE**, en le disant. La premiere version se taisait
// dans ce cas, et son silence devenait indistinguable d'une panne : le journal du 2026-08-31
// contient 733 lignes « la page ne parle plus » dont la plupart n'etaient qu'une fenetre passee
// derriere une autre.
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
    santePage(compte, document.visibilityState === "visible").catch((e) =>
      signalerErreur("sante.rendu", String(e)),
    );
  }, PERIODE);
}
