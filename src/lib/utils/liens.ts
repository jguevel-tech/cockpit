/**
 * Ouverture d'un lien dans le navigateur (ou le client mail) du systeme.
 *
 * Le module existe parce que PLUSIEURS endroits ouvrent des liens — l'editeur de notes, le
 * texte d'une tache — et que la liste blanche des schemas doit dire la MEME chose que la garde
 * Rust `schema_ouvrable` (src-tauri/src/lib.rs). Deux copies divergent : c'est ce qui avait
 * fait refuser un `mailto:` legitime cote backend apres l'avoir accepte cote interface, avec
 * un message technique a l'ecran.
 *
 * Le reperage et le tri des adresses sont dans `adresses.ts` (pur, donc testable sous node).
 */
import { analyserLien, SCHEMAS_OUVRABLES } from "./adresses";
import { openUrl } from "../api/workspace";
import { notify } from "../stores/toast";
import { signalerErreur } from "../stores/errors";
import { translate } from "../i18n";

/**
 * Ouvre un href dans le navigateur (ou le client mail) du systeme.
 *
 * Un refus est toujours DIT : un clic sans effet visible est vecu comme un bug. `scope` situe
 * la panne dans les journaux (`"notes.ouvrirLien"`, `"todos.ouvrirLien"`).
 */
export async function ouvrirLien(href: string, scope: string): Promise<void> {
  const cible = analyserLien(href);
  if (cible === "incomplet") {
    notify(translate("link.incomplete"), "info", 5000, { report: false });
    return;
  }
  if (cible === "illisible") {
    notify(translate("link.invalid", { href }), "error", 4000, { scope });
    return;
  }
  if (!SCHEMAS_OUVRABLES.includes(cible.protocol)) {
    notify(translate("link.refused"), "info", 5000, { report: false });
    return;
  }
  try {
    // L'adresse ABSOLUE, pas le href brut : c'est la seule que le systeme sait ouvrir.
    await openUrl(cible.href);
  } catch (e) {
    signalerErreur(scope, String(e));
    notify(String(e), "error", 4000, { report: false });
  }
}
