/**
 * Coloration des blocs de code d'une note : quelle langue, et quels morceaux peindre.
 *
 * A part du composant pour s'eprouver sous node. La peinture elle-meme passe par l'API des
 * surlignages CSS (`CSS.highlights`), qui ne touche PAS au DOM edite : ni le curseur, ni la
 * conversion en Markdown ne voient la couleur.
 */

/// Langue d'un bloc de code qui n'en declare aucune, ou null si rien n'est sur.
///
/// On se trompe moins en ne coloriant pas qu'en coloriant faux : un texte ordinaire dans un
/// bloc de code reste sans couleur. Le choix explicite, dans la barre d'outils, prime toujours.
export function devinerLangue(code: string): string | null {
  const t = code.trim();
  if (!t) return null;
  if (/^[{[]/.test(t)) {
    try { JSON.parse(t); return "json"; } catch { /* incomplet pendant la frappe : on regarde la forme */ }
    if (/"[^"\n]*"\s*:/.test(t)) return "json";
  }
  if (t.startsWith("<?php")) return "php";
  if (/^<(!doctype|html|head|body|div|span|p|a|ul|li|table|section|template)\b/i.test(t)) return "html";
  if (t.startsWith("<")) return "xml";
  if (/^(select|insert|update|delete|create|alter|drop|with|explain)\s/i.test(t)) return "sql";
  if (/^(\$ |#!\/|sudo |git |npm |npx |docker |kubectl |cd |ls |curl |cat |export |composer |php bin\/)/.test(t)) return "shellscript";
  if (/^(FROM|ARG) \S/.test(t) && /^(RUN|COPY|CMD|ENTRYPOINT|WORKDIR) /m.test(t)) return "dockerfile";
  const lignes = t.split("\n").filter((l) => l.trim());
  if (lignes.length > 0
    && lignes.every((l) => /^\s*(- |[\w.-]+:(\s|$)|#|---)/.test(l))
    && lignes.some((l) => /^\s*[\w.-]+:(\s|$)/.test(l))) return "yaml";
  return null;
}

/// Une ouverture de bloc tapee a la main (« ```json »), ou null. Rend la langue, vide si aucune.
export function ouvertureDeBloc(ligne: string): string | null {
  const m = ligne.trim().match(/^```([\w+#.-]*)$/);
  return m ? m[1].toLowerCase() : null;
}

/// Un morceau colore d'un bloc de code, en positions dans son texte.
export interface Segment {
  debut: number;
  fin: number;
  couleur: string;
}

/// Des jetons de Shiki (une liste par ligne) aux segments, positions comptees dans le texte
/// ENTIER du bloc, sauts de ligne compris. La couleur par defaut du theme n'est pas reprise :
/// elle repeindrait le texte ordinaire et le rendrait illisible si le theme change.
export function segmentsDepuisJetons(
  lignes: { content: string; color?: string }[][],
  couleurParDefaut: string | undefined,
): Segment[] {
  const sortie: Segment[] = [];
  let position = 0;
  lignes.forEach((ligne, i) => {
    if (i > 0) position += 1;
    for (const jeton of ligne) {
      const fin = position + jeton.content.length;
      const couleur = jeton.color?.toLowerCase();
      if (couleur && couleur !== couleurParDefaut?.toLowerCase() && jeton.content.trim()) {
        sortie.push({ debut: position, fin, couleur });
      }
      position = fin;
    }
  });
  return sortie;
}
