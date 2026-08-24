import { writable } from "svelte/store";

/** Ce qu'on demande a l'utilisateur d'ecrire. */
export interface Saisie {
  /** La question. Une phrase, deja traduite par l'appelant. */
  message: string;
  /** Libelle du bouton qui valide ("Creer", "Renommer"...). Traduit par l'appelant. */
  action: string;
  /** Ce que le champ contient a l'ouverture. Selectionne, donc remplacable en tapant. */
  valeur: string;
  /** Texte grise du champ vide. */
  exemple: string;
  /** Resout la promesse rendue a l'appelant. Usage interne. */
  repondre: (texte: string | null) => void;
}

export const saisie = writable<Saisie | null>(null);

/**
 * Demande un texte, dans une fenetre de l'application.
 *
 * **Remplace le `prompt()` du navigateur, qui n'existe PAS dans la WebView de macOS.** Le
 * delegate de wry n'implemente pas `runJavaScriptTextInputPanel`, et WKWebView rend alors la
 * main immediatement : `prompt()` vaut `null` sans rien afficher. Un bouton qui appelle
 * `prompt()` ne fait donc RIEN sur un Mac — pas d'erreur, pas de fenetre, rien. Signale par un
 * utilisateur mac le 2026-08-24, invisible sous Linux ou WebKitGTK affiche la fenetre.
 *
 * Meme forme d'appel que `demanderConfirmation()`, au type de retour pres :
 *
 *     const nom = await demanderTexte({ message: ..., action: ... });
 *     if (nom === null) return;   // annule
 *
 * Rend `null` a l'annulation, comme `prompt()`, pour qu'un remplacement ne change rien
 * d'autre. Un texte vide est traite comme une annulation : valider un champ vide ne veut rien
 * dire quand on demande un nom.
 *
 * Une seule fenetre a la fois : une demande qui arrive alors qu'une autre est ouverte REMPLACE
 * la precedente, en l'annulant.
 */
export function demanderTexte(
  demande: Omit<Saisie, "repondre" | "valeur" | "exemple"> & { valeur?: string; exemple?: string },
): Promise<string | null> {
  return new Promise((resolve) => {
    saisie.update((precedente) => {
      precedente?.repondre(null);
      return {
        message: demande.message,
        action: demande.action,
        valeur: demande.valeur ?? "",
        exemple: demande.exemple ?? "",
        repondre: (texte: string | null) => {
          saisie.set(null);
          resolve(texte);
        },
      };
    });
  });
}
