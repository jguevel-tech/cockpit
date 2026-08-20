/**
 * Adresses : reperage dans du texte brut, et tri d'un href avant ouverture.
 *
 * Module PUR, sans aucun import : c'est ce qui permet de le jouer sous node
 * (`npm run test:front`) sans monter l'application ni simuler Tauri. Tout ce qui touche a
 * l'interface — messages, toasts, ouverture reelle — vit dans `liens.ts`.
 */

/**
 * Schemas qu'on accepte d'ouvrir. DOIT rester aligne sur `schema_ouvrable`
 * (src-tauri/src/lib.rs) : le backend refuse tout le reste, et un desaccord entre les deux
 * fabrique une erreur technique sur un lien parfaitement legitime.
 */
export const SCHEMAS_OUVRABLES = ["http:", "https:", "mailto:"];

/**
 * Trie un href en trois cas : adresse absolue, lien incomplet (relatif ou sans schema),
 * href illisible.
 *
 * La distinction compte parce que le message affiche en depend. Resoudre le href contre une
 * base bidon (`new URL(href, "http://lien.invalid")`) faisait passer `www.ex.com` et
 * `../doc.md` pour des liens http valides : la liste blanche les acceptait, puis le backend
 * refusait le href brut et l'utilisateur recevait une erreur technique.
 */
export function analyserLien(href: string): URL | "incomplet" | "illisible" {
  try {
    return new URL(href);
  } catch {
    // Pas absolu. Silence VOLONTAIRE : le cas est traite juste en dessous, pas avale.
  }
  try {
    new URL(href, "http://lien.invalid");
    return "incomplet";
  } catch {
    return "illisible";
  }
}
