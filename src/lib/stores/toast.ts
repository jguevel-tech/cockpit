import { writable } from "svelte/store";

export interface Toast {
  id: number;
  message: string;
  kind: "error" | "success" | "info";
}

export const toasts = writable<Toast[]>([]);

let nextId = 1;

/**
 * Feedback non bloquant — remplace les `catch {}` muets et la plupart des alert().
 * `notify(String(e))` en erreur, `notify("Fait", "success")` en confirmation.
 */
export function notify(message: string, kind: Toast["kind"] = "error", durationMs = 4000) {
  const id = nextId++;
  toasts.update((list) => [...list, { id, message, kind }]);
  setTimeout(() => {
    toasts.update((list) => list.filter((t) => t.id !== id));
  }, durationMs);
}
