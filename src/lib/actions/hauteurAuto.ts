/**
 * Une zone de texte qui grandit avec ce qu'on y ecrit, au lieu d'afficher une barre de
 * defilement des la deuxieme ligne. Le plafond vient du CSS (`max-height`) : au-dela, elle
 * defile.
 */
export function hauteurAuto(el: HTMLTextAreaElement, _valeur?: string) {
  const ajuster = () => {
    el.style.height = "auto";
    // `scrollHeight` ne compte pas la bordure : sans elle, la zone manque de deux pixels et
    // affiche une barre de defilement des la premiere ligne (vu au banc).
    el.style.height = `${el.scrollHeight + el.offsetHeight - el.clientHeight}px`;
    // Une ligne de 16,66 px plus le padding font 26,46 px : un demi-pixel de trop, et la barre
    // de defilement apparait sur une zone qui montre pourtant tout. Elle ne sert qu'une fois
    // le plafond atteint (mesure au banc : scrollHeight 26 pour clientHeight 26).
    el.style.overflowY = el.scrollHeight > el.clientHeight + 1 ? "auto" : "hidden";
  };
  el.addEventListener("input", ajuster);
  queueMicrotask(ajuster);
  return {
    // Une valeur posee par le code (champ vide apres un ajout) ne declenche pas `input` :
    // l'appelant passe la valeur en parametre, et chaque changement reajuste.
    update: () => queueMicrotask(ajuster),
    destroy() {
      el.removeEventListener("input", ajuster);
    },
  };
}
