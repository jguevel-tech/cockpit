import { santePage } from "../api/sante";
import { signalerErreur } from "./errors";

// Prouver que la page se PEINT, pas seulement qu'elle tourne.
//
// « Le code tourne » et « l'ecran se met a jour » sont deux choses differentes : un minuteur
// continue de tomber quand le moteur de rendu a cesse de peindre, une demande d'image NON. C'est
// ce qui les separe, et c'est ce qui a permis de nommer le gel du 2026-08-31.
//
// **UNE SEULE DEMANDE D'IMAGE PAR PERIODE, ET C'EST UNE CORRECTION.** La premiere version
// relancait une demande a CHAQUE image, donc soixante fois par seconde, et empechait la page de
// se reposer : interface plus lente et lettres qui sautaient en cours de frappe. Une demande
// toutes les cinq secondes repond a la meme question — le moteur peint-il encore ? — pour trois
// centiemes du cout.
//
// **ON PARLE MEME QUAND LA FENETRE EST CACHEE**, en le disant : une page cachee ne peint pas et
// ce n'est pas une panne. La version qui se taisait rendait son silence indistinguable d'un gel.
const PERIODE = 5000;

let aPeint = false;
let demandeEnCours = false;

function demanderUneImage() {
  if (demandeEnCours) return;
  demandeEnCours = true;
  requestAnimationFrame(() => {
    aPeint = true;
    demandeEnCours = false;
  });
}

export function surveillerLeRendu() {
  demanderUneImage();

  setInterval(() => {
    const peint = aPeint;
    aPeint = false;
    santePage(peint, document.visibilityState === "visible").catch((e) =>
      signalerErreur("sante.rendu", String(e)),
    );
    demanderUneImage();
  }, PERIODE);
}
