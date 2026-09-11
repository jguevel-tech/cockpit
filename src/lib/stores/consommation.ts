import { writable, get } from "svelte/store";
import { lireConsommation, type EtatConsommation } from "../api/consommation";
import { signalerErreur } from "./errors";

/// La consommation du fournisseur choisi, relue de temps en temps.
///
/// **CE QUI EST APPELE SUR UN MINUTEUR SE MESURE.** Un appel reseau toutes les dix minutes,
/// et seulement quand la fenetre est visible : en arriere-plan, personne ne lit la jauge et
/// reveiller le reseau pour rien serait payer sans rien rendre. La regle du projet vaut ici
/// comme pour la surveillance des conteneurs.
const PERIODE_MS = 10 * 60 * 1000;

/// Deux appels de suite ne servent a rien : le fournisseur ne recompte pas si vite.
const FRAICHEUR_MS = 30 * 1000;

export const consommation = writable<EtatConsommation | null>(null);

let derniereLecture = 0;
let enCours = false;

/// Relit la consommation. `force` ignore la fraicheur : c'est le geste explicite de quelqu'un
/// qui ouvre le panneau et veut un chiffre a jour.
export async function rafraichirConsommation(force = false): Promise<void> {
  if (enCours) return;
  if (!force && Date.now() - derniereLecture < FRAICHEUR_MS) return;
  enCours = true;
  try {
    const etat = await lireConsommation();
    derniereLecture = Date.now();
    consommation.set(etat);
  } catch (e) {
    // Le magasin garde sa derniere valeur : un chiffre d'il y a dix minutes vaut mieux qu'une
    // jauge qui disparait a la premiere coupure reseau.
    void signalerErreur("consommation.lecture", String(e));
  } finally {
    enCours = false;
  }
}

/// Demarre le suivi et rend de quoi l'arreter.
export function suivreLaConsommation(): () => void {
  void rafraichirConsommation(true);
  const minuteur = setInterval(() => {
    if (document.visibilityState === "visible") void rafraichirConsommation();
  }, PERIODE_MS);
  // Au retour sur la fenetre : le chiffre a peut-etre vieilli pendant l'absence.
  const auRetour = () => {
    if (document.visibilityState === "visible") void rafraichirConsommation();
  };
  document.addEventListener("visibilitychange", auRetour);
  return () => {
    clearInterval(minuteur);
    document.removeEventListener("visibilitychange", auRetour);
  };
}

/// La fenetre la plus serree : c'est elle qui decide quand on sera coupe, donc c'est elle que
/// la jauge de l'en-tete montre.
export function fenetreLaPlusServree(etat: EtatConsommation | null) {
  if (!etat || etat.fenetres.length === 0) return null;
  return etat.fenetres.reduce((pire, f) => (f.pourcentage > pire.pourcentage ? f : pire));
}

/// Y a-t-il quelque chose a montrer ? Un fournisseur sans cette capacite, ou une lecture qui
/// n'a rien rendu, ne donne PAS une jauge vide : elle se lirait « consommation nulle ».
export function aQuelqueChoseAMontrer(etat: EtatConsommation | null): boolean {
  return !!etat && etat.gere && etat.fenetres.length > 0;
}

/// Relit tout de suite si le fournisseur choisi a change : la jauge parle d'un fournisseur
/// nomme, elle ne doit pas garder les chiffres du precedent.
export function oublierLaConsommation(): void {
  derniereLecture = 0;
  if (get(consommation)) consommation.set(null);
  void rafraichirConsommation(true);
}
