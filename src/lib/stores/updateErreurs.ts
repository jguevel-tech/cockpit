/**
 * Comment lire l'echec d'une mise a jour.
 *
 * Separe de `update.ts` parce que ce sont des REGLES et non de l'orchestration : sans aucune
 * dependance, elles s'essaient directement sous node. `update.ts` tire les modules Tauri, qui
 * n'existent pas la-bas.
 *
 * La regle qui gouverne tout ce fichier : **un « impossible » sans raison n'est pas un
 * message**. Il laisse l'utilisateur sans rien a faire, et nous sans rien a lire.
 */
import type { Catalog } from "../i18n/fr";

/**
 * Traduit une panne technique en cle d'affichage, quand on sait la nommer.
 *
 * Une CLE et non un texte : le message reste reactif au changement de langue la ou il est
 * affiche. Rend `null` quand aucun motif ne correspond — c'est alors `detailErreurMaj` qui
 * prend le relais.
 */
export function cleErreurMaj(brut: string): keyof Catalog | null {
  // Une release existe mais l'artefact de notre systeme n'y est pas encore : les jobs de
  // plateformes ne finissent pas en meme temps. Ce n'est pas une panne, c'est « repasse ».
  if (/platforms` object/.test(brut)) return "update.notReady";

  // Sous Linux la mise a jour remplace le fichier de l'application LA OU IL EST. Deplace ou
  // renomme depuis l'installation, ce fichier n'existe plus a l'adresse que le systeme
  // annonce. Le motif porte sur le NUMERO de l'erreur, pas sur son texte : celui-ci est
  // traduit par le systeme, donc « No such file or directory » chez les uns et « Aucun
  // fichier ou dossier de ce nom » chez les autres.
  if (/os error 2\b/.test(brut) || /No such file or directory/i.test(brut)) return "update.moved";

  if (/error sending request|dns error|timed out|timeout|connection|connect |unreachable|network/i.test(brut)) {
    return "update.offline";
  }
  return null;
}

/**
 * Le detail technique a montrer quand on n'a pas su nommer la panne.
 *
 * Rend `null` des qu'un motif connu explique deja l'echec : un message clair suivi d'une trace
 * technique fait douter du message.
 */
export function detailErreurMaj(brut: string): string | null {
  return cleErreurMaj(brut) === null ? brut.trim().slice(0, 300) : null;
}
