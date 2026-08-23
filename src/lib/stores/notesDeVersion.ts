/**
 * Mise en langue des notes d'une release.
 *
 * Les notes affichees dans la cloche sont le CHANGELOG tel qu'il est publie. Son contenu est
 * ecrit en francais et en anglais selon la langue de lecture... sauf ses titres de section, qui
 * suivent la convention Keep a Changelog et restent en anglais dans le fichier. Un lecteur
 * francais voyait donc « Added » au-dessus de puces francaises.
 *
 * Separe du composant parce que ce sont des REGLES : sans aucune dependance, elles s'essaient
 * directement sous node.
 */
import type { Catalog } from "../i18n/fr";

/// Les sections de Keep a Changelog, en minuscules, vers leur cle d'affichage. `unreleased`
/// est du meme registre : c'est un nom fixe de la convention, pas un contenu.
const SECTIONS: Record<string, keyof Catalog> = {
  unreleased: "changelog.unreleased",
  added: "changelog.added",
  changed: "changelog.changed",
  deprecated: "changelog.deprecated",
  removed: "changelog.removed",
  fixed: "changelog.fixed",
  security: "changelog.security",
};

/**
 * Rend les notes avec leurs titres de section dans la langue de lecture.
 *
 * Ne touche QUE les lignes de titre dont le texte est exactement un des six noms de section :
 * le reste des notes est du contenu qu'on n'a pas a reecrire. Le niveau du titre est conserve,
 * et les blocs de code sont laisses tels quels — un `### Added` cite dans un exemple reste un
 * exemple.
 *
 * @param dire le traducteur de la langue courante (`$trad` dans un composant).
 */
export function titresEnLangue(notes: string, dire: (cle: keyof Catalog) => string): string {
  let dansUnBloc = false;
  return notes
    .split("\n")
    .map((ligne) => {
      if (/^\s*(```|~~~)/.test(ligne)) {
        dansUnBloc = !dansUnBloc;
        return ligne;
      }
      if (dansUnBloc) return ligne;
      // Les crochets sont optionnels : la convention ecrit « ## [Unreleased] » mais
      // « ### Added ». Un titre de VERSION porte aussi des crochets et n'est pas touche,
      // parce que son texte n'est pas un nom de section connu.
      const titre = /^(#{1,6})\s+\[?([A-Za-z]+)\]?\s*$/.exec(ligne);
      if (!titre) return ligne;
      const cle = SECTIONS[titre[2].toLowerCase()];
      return cle ? `${titre[1]} ${dire(cle)}` : ligne;
    })
    .join("\n");
}

/**
 * Coupe les notes apres les `combien` premieres sections de version.
 *
 * Pourquoi : le changelog est affiche en entier dans les Parametres, et il ne fait que
 * grossir — 85 versions, 57 Ko, ~40 ms de rendu et 62 Ko de HTML poses dans la page a chaque
 * ouverture. Les deux dernieres versions font 1 % du fichier. On rend donc le debut, et le
 * reste seulement si on le demande.
 *
 * Rend `reste` vide quand il n'y a pas assez de sections : l'appelant n'a alors rien de plus a
 * proposer.
 */
export function couperLesNotes(notes: string, combien: number): { tete: string; reste: string } {
  const debuts: number[] = [];
  let position = 0;
  for (const ligne of notes.split("\n")) {
    if (/^## \[/.test(ligne)) debuts.push(position);
    position += ligne.length + 1;
  }
  if (debuts.length <= combien) return { tete: notes, reste: "" };
  const coupure = debuts[combien];
  return { tete: notes.slice(0, coupure), reste: notes.slice(coupure) };
}
