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
//
// **L'ENTREE UTILISATEUR NE MEMORISE QUE SON MOMENT.** Ni le texte frappe, ni la position : un
// horodatage. Le guetteur s'en sert pour decider si quelqu'un est devant la fenetre avant de la
// recharger — les episodes d'ecran eteint, la nuit, ne doivent declencher aucune reparation.
const PERIODE = 5000;

/// Fenetre de presence cote page : une entree compte pendant deux minutes. Le guetteur
/// elargit de son cote (cinq tours) pour couvrir un rapport manque.
const FENETRE_ENTREE = 120_000;

let aPeint = false;
let demandeEnCours = false;
let derniereEntree = 0;

function marquerEntree() {
  derniereEntree = Date.now();
}

function demanderUneImage() {
  if (demandeEnCours) return;
  demandeEnCours = true;
  requestAnimationFrame(() => {
    aPeint = true;
    demandeEnCours = false;
  });
}

export function surveillerLeRendu() {
  // Capture + passifs : on ne filtre rien et on ne ralentit aucun geste, xterm compris.
  window.addEventListener("keydown", marquerEntree, { capture: true, passive: true });
  window.addEventListener("pointerdown", marquerEntree, { capture: true, passive: true });

  demanderUneImage();

  setInterval(() => {
    const peint = aPeint;
    aPeint = false;
    const entreeRecente =
      derniereEntree > 0 && Date.now() - derniereEntree < FENETRE_ENTREE;
    santePage(peint, document.visibilityState === "visible", entreeRecente).catch(
      (e) => signalerErreur("sante.rendu", String(e)),
    );
    demanderUneImage();
  }, PERIODE);
}
