import { writable } from "svelte/store";
import { signalerErreur } from "./errors";

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
 *
 * TOUTE erreur affichee ici est aussi JOURNALISEE et, si l'utilisateur l'a accepte,
 * remontee au serveur de suivi. C'est volontairement branche a cet endroit : c'est le
 * passage oblige des erreurs de l'interface, donc rien n'est oublie et une nouvelle
 * fonctionnalite est couverte sans y penser.
 *
 * `opts.scope` situe la panne ("git.commit") et rend la remontee exploitable.
 * `opts.report: false` pour une erreur attendue qui n'apprend rien (saisie invalide).
 */
export function notify(
  message: string,
  kind: Toast["kind"] = "error",
  durationMs = 4000,
  opts: { scope?: string; report?: boolean } = {},
) {
  if (opts.report ?? kind === "error") {
    void signalerErreur(opts.scope ?? "interface", message);
  }
  const id = nextId++;
  toasts.update((list) => [...list, { id, message, kind }]);
  setTimeout(() => {
    toasts.update((list) => list.filter((t) => t.id !== id));
  }, durationMs);
}
