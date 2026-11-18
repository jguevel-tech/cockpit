import { invoke } from "../coquille";

/**
 * Journal des erreurs.
 *
 * Le point d'entree unique est `signalerErreur`, appele automatiquement par `notify(...,
 * "error")` : toute erreur affichee a l'utilisateur est donc journalisee, sans avoir a
 * penser a l'instrumenter. Le journal est LOCAL (`logs/cockpit.log`) : rien ne quitte la
 * machine.
 *
 * Ne rejette jamais : une journalisation qui echoue ne doit pas ajouter une erreur a l'erreur.
 * `scope` situe la panne ("git.commit", "recorder.start").
 */
export async function signalerErreur(scope: string, message: string) {
  try {
    await invoke("report_error", { scope, message });
  } catch (e) {
    console.warn("journal d'erreurs impossible :", String(e));
  }
}
