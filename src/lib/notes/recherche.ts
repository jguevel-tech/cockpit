/**
 * Recherche dans les notes d'un projet, et dans la note ouverte.
 *
 * A part du composant pour s'eprouver sous node. Aucun DOM ici : l'editeur fait le lien entre
 * ces positions et ses noeuds texte.
 */

/// Un caractere replie : sans accent, en minuscule, et TOUJOURS d'une seule unite.
///
/// **LA LONGUEUR NE DOIT PAS CHANGER.** Les positions trouvees dans le texte replie servent a
/// surligner le texte d'origine : un « é » qui deviendrait deux unites (e + accent) decalerait
/// tout ce qui suit. On garde donc le premier caractere de chaque decomposition.
function plierUn(c: string): string {
  const sansAccent = c.normalize("NFD")[0] ?? c;
  const minuscule = sansAccent.toLowerCase();
  return minuscule.length === 1 ? minuscule : sansAccent;
}

/// Le texte replie, de la meme longueur que l'original : « Résultat » trouve « resultat ».
export function plier(texte: string): string {
  let sortie = "";
  for (let i = 0; i < texte.length; i++) sortie += plierUn(texte[i]);
  return sortie;
}

/// Positions de debut de chaque occurrence, sans chevauchement, bornees a `max`.
export function occurrences(texte: string, requete: string, max = 2000): number[] {
  const aiguille = plier(requete.trim());
  if (!aiguille) return [];
  const botte = plier(texte);
  const sortie: number[] = [];
  let i = botte.indexOf(aiguille);
  while (i !== -1 && sortie.length < max) {
    sortie.push(i);
    i = botte.indexOf(aiguille, i + aiguille.length);
  }
  return sortie;
}

/// Un morceau de la ligne qui contient la premiere occurrence, decoupe en trois pour que
/// l'interface mette le mot trouve en evidence sans passer par du HTML.
export interface Extrait {
  avant: string;
  trouve: string;
  apres: string;
}

const MARGE_EXTRAIT = 40;

export function extrait(texte: string, requete: string): Extrait | null {
  const [debut] = occurrences(texte, requete, 1);
  if (debut === undefined) return null;
  const longueur = requete.trim().length;
  const debutLigne = texte.lastIndexOf("\n", debut) + 1;
  const finLigne = texte.indexOf("\n", debut + longueur);
  const fin = finLigne === -1 ? texte.length : finLigne;
  const de = Math.max(debutLigne, debut - MARGE_EXTRAIT);
  const a = Math.min(fin, debut + longueur + MARGE_EXTRAIT);
  return {
    avant: (de > debutLigne ? "…" : "") + texte.slice(de, debut).trimStart(),
    trouve: texte.slice(debut, debut + longueur),
    apres: texte.slice(debut + longueur, a).trimEnd() + (a < fin ? "…" : ""),
  };
}

export interface NoteCherchable {
  id: number;
  name: string;
  content: string;
}

export interface Resultat<N extends NoteCherchable> {
  note: N;
  dansLeNom: boolean;
  /// Occurrences dans le CONTENU, le nom n'est pas compte.
  nombre: number;
  extrait: Extrait | null;
}

/// Les notes qui contiennent la requete, dans leur nom ou leur texte.
///
/// Un nom trouve passe devant : on cherche souvent une note dont on se rappelle le titre.
/// Ensuite, la note qui en parle le plus.
export function chercherDansLesNotes<N extends NoteCherchable>(notes: N[], requete: string): Resultat<N>[] {
  if (!requete.trim()) return [];
  const sortie: Resultat<N>[] = [];
  for (const note of notes) {
    const dansLeNom = occurrences(note.name, requete, 1).length > 0;
    const nombre = occurrences(note.content, requete).length;
    if (!dansLeNom && nombre === 0) continue;
    sortie.push({ note, dansLeNom, nombre, extrait: nombre > 0 ? extrait(note.content, requete) : null });
  }
  return sortie.sort((a, b) => Number(b.dansLeNom) - Number(a.dansLeNom) || b.nombre - a.nombre);
}
