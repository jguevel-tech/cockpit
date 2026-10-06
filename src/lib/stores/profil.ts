import { writable } from "svelte/store";
import { profilsDesFenetres } from "../coquille";

/** Le dialogue « ouvrir une fenetre » est-il affiche. */
export const choixDeProfilOuvert = writable(false);

/** Le profil de cette fenetre. `null` = profil par defaut, qu'on ne nomme pas a l'ecran. */
export const profilCourant = writable<string | null>(null);

export async function chargerLeProfilCourant(): Promise<void> {
  profilCourant.set((await profilsDesFenetres()).courant);
}
