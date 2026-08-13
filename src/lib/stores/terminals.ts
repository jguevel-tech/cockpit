import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { listAllTerminals } from "../api/workspace";
import type { TerminalInfo } from "../types";

// Terminaux vivants (toutes sessions tmux), pour la sidebar et le dashboard.
export const terminals = writable<TerminalInfo[]>([]);

export async function loadTerminals() {
  try {
    terminals.set((await listAllTerminals()).filter((t) => t.alive));
  } catch {}
}

loadTerminals();

// Un shell qui se termine disparait de la liste
listen("terminal_exit", () => {
  loadTerminals();
});

// Le flag `llm` (agent IA en cours dans la session) evolue pendant la vie du
// terminal : refresh periodique leger (1 appel tmux + 1 ps cote backend).
setInterval(loadTerminals, 5000);
