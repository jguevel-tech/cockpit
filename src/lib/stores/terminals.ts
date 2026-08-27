import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { listAllTerminals } from "../api/workspace";
import type { TerminalInfo } from "../types";
import { signalerErreur } from "./errors";

// Terminaux vivants (toutes sessions du serveur), pour la sidebar et le dashboard.
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
    terminals.set((await listAllTerminals()).filter((t) => t.alive));
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
