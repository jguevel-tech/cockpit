import { writable } from "svelte/store";

/** Ce qu'on demande a l'utilisateur de confirmer. */
export interface Confirmation {
  /** Le texte de la question. Une phrase, deja traduite par l'appelant. */
  message: string;
  /** Libelle du bouton qui valide. Traduit par l'appelant ("Supprimer", "Abandonner"...). */
  action: string;
  /** Action destructive : le bouton de validation passe en rouge. */
  danger: boolean;
  /** Resout la promesse rendue a l'appelant. Usage interne. */
  repondre: (accepte: boolean) => void;
}

export const confirmation = writable<Confirmation | null>(null);

/**
 * Demande une confirmation, dans une fenetre de l'application.
 *
 * Remplace le `confirm()` du systeme, qui ne suit ni le theme ni la langue, et dont
 * l'apparence n'a rien a voir avec le reste du logiciel. La forme d'appel est volontairement
 * la meme, au `await` pres, pour qu'un remplacement ne change rien d'autre :
 *
 *     if (!(await demanderConfirmation({ message: ..., action: ... }))) return;
 *
 * Une seule fenetre a la fois : une demande qui arrive alors qu'une autre est ouverte
 * REMPLACE la precedente, en la refusant. Empiler deux questions ne veut rien dire pour
 * l'utilisateur, et laisser la premiere en attente la ferait resurgir plus tard sans contexte.
 */
export function demanderConfirmation(
  demande: Omit<Confirmation, "repondre" | "danger"> & { danger?: boolean },
): Promise<boolean> {
  return new Promise((resolve) => {
    confirmation.update((precedente) => {
      precedente?.repondre(false);
      return {
        message: demande.message,
        action: demande.action,
        danger: demande.danger ?? true,
        repondre: (accepte: boolean) => {
          confirmation.set(null);
          resolve(accepte);
        },
      };
    });
  });
}
