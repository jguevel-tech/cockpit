import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { listAllTerminals } from "../api/workspace";
import type { TerminalInfo } from "../types";
import { signalerErreur } from "./errors";

// Terminaux vivants (toutes sessions du serveur), pour la sidebar et le dashboard.
export const terminals = writable<TerminalInfo[]>([]);

export async function loadTerminals() {
  try {
    terminals.set((await listAllTerminals()).filter((t) => t.alive));
  } catch (e) {
      signalerErreur("terminals.loadTerminals", String(e));}
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
