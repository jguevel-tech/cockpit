import { santePage } from "../api/sante";
import { signalerErreur } from "./errors";

// Prouver que la page se PEINT, pas seulement qu'elle tourne.
//
// « Le code tourne » et « l'ecran se met a jour » sont deux choses differentes : un minuteur
// continue de tomber quand le moteur de rendu a cesse de peindre, une demande d'image NON. C'est
// ce qui les separe, et c'est ce qui a permis de nommer le gel du 2026-08-31.
//
// **UNE SEULE DEMANDE D'IMAGE PAR RAPPORT.** La premiere version relancait une demande a CHAQUE
// image, donc soixante fois par seconde, et empechait la page de se reposer : interface plus
// lente et lettres qui sautaient en cours de frappe. Une demande par rapport repond a la meme
// question — le moteur peint-il encore ? — pour trois centiemes du cout.
//
// **LE RAPPORT S'ACCELERE PENDANT LE DOUTE, ET C'EST CE QUI RACCOURCIT LES GELS.** Toutes les
// cinq secondes quand tout va bien ; toutes les secondes des qu'un rapport dit « visible,
// concentree, rien n'est peint ». Le guetteur confirme une panne sur trois rapports : au rythme
// lent la confirmation prenait quinze secondes, et le gel du 2026-09-09 a ete tue par
// l'utilisateur avant que la reparation n'arrive. Au rythme accelere elle prend trois secondes.
// Le cout ne tombe que pendant le doute : une page qui peint reste a une demande par seconde
// au pire, et l'acceleration s'arrete des qu'une image revient.
//
// **LE FOCUS EST RAPPORTE, ET IL DISCULPE LA FENETRE RECOUVERTE.** Une fenetre sous une autre
// garde `visibilityState = "visible"` et cesse de produire des images : sans le focus, travailler
// ailleurs faisait accuser le moteur de rendu (journal du 2026-08-31 plein de ces alternances).
//
// **ON PARLE MEME QUAND LA FENETRE EST CACHEE**, en le disant : une page cachee ne peint pas et
// ce n'est pas une panne. La version qui se taisait rendait son silence indistinguable d'un gel.
//
// **L'ENTREE UTILISATEUR NE MEMORISE QUE SON MOMENT.** Ni le texte frappe, ni la position : un
// horodatage. Le guetteur s'en sert pour decider si quelqu'un est devant la fenetre avant de la
// recharger — les episodes d'ecran eteint, la nuit, ne doivent declencher aucune reparation.
const PERIODE = 5000;
const PERIODE_DOUTE = 1000;

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

  // Une chaine de `setTimeout` et non un `setInterval` : le delai du prochain rapport
  // depend de ce que dit le precedent.
  function boucle() {
    const peint = aPeint;
    aPeint = false;
    const visible = document.visibilityState === "visible";
    const concentre = document.hasFocus();
    const entreeRecente =
      derniereEntree > 0 && Date.now() - derniereEntree < FENETRE_ENTREE;
    santePage(peint, visible, concentre, entreeRecente).catch((e) =>
      signalerErreur("sante.rendu", String(e)),
    );
    demanderUneImage();
    const doute = visible && concentre && !peint;
    setTimeout(boucle, doute ? PERIODE_DOUTE : PERIODE);
  }

  setTimeout(boucle, PERIODE);
}
