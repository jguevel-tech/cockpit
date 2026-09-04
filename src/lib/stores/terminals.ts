import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { listAllTerminals } from "../api/workspace";
import type { TerminalInfo } from "../types";
import { signalerErreur } from "./errors";

// TOUS les terminaux, tous projets confondus, pour la barre laterale et le tableau de bord.
//
// **PAS SEULEMENT LES VIVANTS, ET C'EST LE CORRECTIF DE LA 0.56.1.** Un terminal dont le
// service n'a plus la session n'est pas perdu : son shell est mort avec la machine, sa ligne
// et l'ecran qu'il affichait ont survecu, et l'ouvrir le rouvre. Le filtre qui vivait ici
// rendait donc la liste VIDE au premier demarrage suivant une extinction, et on croyait ses
// terminaux perdus alors qu'ils etaient tous en base.
export const terminals = writable<TerminalInfo[]>([]);

// Un appel deja en vol interdit le suivant. Sans cette garde, un service qui met plus de
// cinq secondes a repondre voit les appels S'EMPILER — un de plus toutes les cinq secondes,
// chacun tenant un fil du backend. Quand il n'en reste plus, AUCUNE commande ne repond et
// l'interface entiere semble morte.
let enVol = false;

export async function loadTerminals() {
  if (enVol) return;
  enVol = true;
  try {
    terminals.set(await listAllTerminals());
  } catch (e) {
    signalerErreur("terminals.loadTerminals", String(e));
  } finally {
    enVol = false;
  }
}

loadTerminals();

// Un shell qui se termine disparait de la liste
listen("terminal_exit", () => {
  loadTerminals();
});

// Le flag `llm` (agent IA en cours dans la session) evolue pendant la vie du
// terminal : refresh periodique leger (une question au service, qui construit UN seul
// arbre de process pour toutes les sessions).
setInterval(loadTerminals, 5000);
