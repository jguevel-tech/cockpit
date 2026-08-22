/**
 * Les refus que le serveur peut rendre sur un compte, et la phrase a montrer.
 *
 * Une TABLE et non une cle construite a la volee : le catalogue est type, donc ecrire les cles
 * en toutes lettres fait verifier par le compilateur qu'elles existent — dans les deux langues.
 * Une cle assemblee passerait la verification et manquerait a l'affichage.
 *
 * Partagee entre l'ecran de connexion et le profil : deux copies auraient fini par diverger, et
 * ce sont des textes que l'utilisateur lit.
 */
import type { Catalog } from "../i18n/fr";

const REFUS = {
  identifiants_invalides: "compte.refus.identifiants_invalides",
  adresse_deja_prise: "compte.refus.adresse_deja_prise",
  adresse_invalide: "compte.refus.adresse_invalide",
  mot_de_passe_trop_court: "compte.refus.mot_de_passe_trop_court",
  mots_de_passe_differents: "compte.refus.mots_de_passe_differents",
  trop_de_tentatives: "compte.refus.trop_de_tentatives",
  reseau: "compte.refus.reseau",
  appairage_expire: "compte.refus.appairage_expire",
  pas_connecte: "compte.refus.pas_connecte",
  serveur_non_chiffre: "settings.compte.serveurNonChiffre",
  avatar_trop_gros: "compte.refus.avatar_trop_gros",
  avatar_format_refuse: "compte.refus.avatar_format_refuse",
  avatar_dimensions_refusees: "compte.refus.avatar_trop_grande",
  avatar_illisible: "compte.refus.avatar_illisible",
  avatar_vide: "compte.refus.avatar_illisible",
} as const;

/**
 * La cle a afficher pour un motif. Un motif inconnu tombe sur un message general : le serveur
 * peut en ajouter avant que le logiciel les connaisse, et montrer une cle technique serait pire
 * que rien.
 */
export function texteDuRefus(motif: string | null): keyof Catalog | null {
  if (motif === null) return null;

  return motif in REFUS ? REFUS[motif as keyof typeof REFUS] : "compte.refus.serveur";
}
