/**
 * Le fournisseur d'IA choisi, pour toute l'interface.
 *
 * **UN SEUL ENDROIT LE SAIT.** Le bouton du terminal, l'onglet Plugins et les reglages lisent
 * ce magasin : sans lui, chacun redemanderait le catalogue au demarrage et un changement de
 * choix n'en rafraichirait qu'une partie — deux ecrans affichant deux fournisseurs differents.
 *
 * `null` veut dire « pas encore lu », pas « aucun » : il y a toujours un fournisseur choisi
 * cote Rust. Les composants affichent donc leur etat de chargement, ils ne concluent pas a
 * l'absence.
 */
import { get, writable } from "svelte/store";
import { catalogueLlm, type CapacitesLlm } from "../api/llm";
import { signalerErreur } from "./errors";

export const catalogue = writable<CapacitesLlm[]>([]);
export const agentPrefere = writable<CapacitesLlm | null>(null);

/**
 * Relit le catalogue. A appeler au demarrage, et apres tout changement de choix ou de cle.
 *
 * Une lecture qui echoue laisse l'etat precedent en place et REMONTE l'erreur : vider le
 * catalogue ferait disparaitre le bouton des conversations sans dire pourquoi.
 */
export async function rafraichirLlm(): Promise<void> {
  try {
    const liste = await catalogueLlm();
    catalogue.set(liste);
    agentPrefere.set(liste.find((f) => f.prefere) ?? null);
  } catch (e) {
    signalerErreur("llm.catalogue", String(e));
  }
}

/** Le fournisseur choisi, sans passer par une souscription. */
export function agentPrefereMaintenant(): CapacitesLlm | null {
  return get(agentPrefere);
}
