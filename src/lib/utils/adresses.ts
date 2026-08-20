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

/** Morceau de texte, ou adresse reperee dedans. */
export type Segment =
  | { kind: "texte"; texte: string }
  | { kind: "lien"; texte: string; href: string };

/**
 * Adresses reperables dans du texte brut : `http://`, `https://`, et les adresses mail.
 *
 * Volontairement etroit — pas de `www.` sans schema, pas de `ftp:` : tout ce qui est repere
 * ici doit etre ouvrable tel quel (voir SCHEMAS_OUVRABLES), sinon on souligne des liens qui
 * refuseront de s'ouvrir. Les parentheses restent DANS la classe de l'adresse : elles font
 * partie de beaucoup d'URLs (Wikipedia, Jira), c'est `rognerPonctuation` qui tranche.
 */
const MOTIF_LIEN = /https?:\/\/[^\s<>"'`]+|[\w.+-]+@[a-z\d-]+(?:\.[a-z\d-]+)+/gi;

/** Ponctuation qui termine une phrase, jamais une adresse. */
const PONCTUATION_FINALE = ".,;:!?…\"'’”»";

/** Fermantes dont on ne rogne que les exemplaires non apparies. */
const PAIRES: Record<string, string> = { ")": "(", "]": "[", "}": "{" };

function compter(texte: string, caractere: string): number {
  let n = 0;
  for (const c of texte) if (c === caractere) n++;
  return n;
}

/**
 * Retire la ponctuation de fin de phrase collee a une adresse.
 *
 * « va voir https://exemple.com. » : le point n'est pas dans l'URL. Mais
 * `https://fr.wikipedia.org/wiki/Deja_vu_(homonymie)` en contient une, appariee : on ne rogne
 * une fermante que s'il y en a plus que d'ouvrantes.
 */
function rognerPonctuation(brut: string): string {
  let fin = brut.length;
  while (fin > 0) {
    const c = brut[fin - 1];
    const ouvrante = PAIRES[c];
    if (ouvrante !== undefined) {
      const candidat = brut.slice(0, fin);
      if (compter(candidat, c) <= compter(candidat, ouvrante)) break;
      fin--;
      continue;
    }
    if (PONCTUATION_FINALE.includes(c)) {
      fin--;
      continue;
    }
    break;
  }
  return brut.slice(0, fin);
}

/**
 * Decoupe un texte brut en morceaux de texte et adresses.
 *
 * La concatenation des `texte` rend TOUJOURS le texte d'origine : c'est ce qui garantit qu'on
 * n'affiche ni plus ni moins que ce que l'utilisateur a saisi (un test le verrouille).
 */
export function segmenterLiens(texte: string): Segment[] {
  const segments: Segment[] = [];
  let curseur = 0;
  for (const trouvaille of texte.matchAll(MOTIF_LIEN)) {
    const debut = trouvaille.index;
    const adresse = rognerPonctuation(trouvaille[0]);
    // Une adresse entierement rognee n'existe pas (le motif exige des caracteres utiles),
    // mais on ne veut surtout pas emettre un segment vide qui decalerait le curseur.
    if (!adresse) continue;
    if (debut > curseur) segments.push({ kind: "texte", texte: texte.slice(curseur, debut) });
    segments.push({
      kind: "lien",
      texte: adresse,
      href: /^https?:\/\//i.test(adresse) ? adresse : `mailto:${adresse}`,
    });
    curseur = debut + adresse.length;
  }
  if (curseur < texte.length) segments.push({ kind: "texte", texte: texte.slice(curseur) });
  return segments;
}
