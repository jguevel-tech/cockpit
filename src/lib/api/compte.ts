import { invoke } from "@tauri-apps/api/core";

/** L'etat du compte tel que le backend le voit. */
export type EtatCompte = {
  connecte: boolean;
  email: string | null;
  nom: string | null;
  /** Une ou deux lettres, a afficher quand il n'y a pas d'image. */
  initiales: string | null;
  /** Adresse `data:` de l'avatar, gardee en local pour marcher hors connexion. */
  avatar: string | null;
  serveur: string;
  /** Nom sous lequel cette machine apparait dans la liste du compte. */
  appareil: string;
};

export type DemandeAppairage = {
  id: string;
  code: string;
  url: string;
};

export type EtatAppairage =
  | { etat: "en_attente" }
  | { etat: "accorde"; compte: EtatCompte };

export const etatCompte = () => invoke<EtatCompte>("compte_etat");

/** Ce que le serveur sait faire. Faux quand on ne peut pas lui demander. */
export const googleDisponible = () => invoke<boolean>("compte_google_disponible");

export const inscription = (email: string, motDePasse: string, nom: string | null) =>
  invoke<EtatCompte>("compte_inscription", { email, motDePasse, nom });

export type Machine = {
  id: string;
  nom: string;
  systeme: string;
  vu_le: string;
};

/** Rend la liste des machines et l'identifiant de celle-ci. */
export const machines = () => invoke<[Machine[], string | null]>("compte_machines");

export const definirNom = (nom: string) => invoke<EtatCompte>("compte_definir_nom", { nom });
export const deposerAvatar = (chemin: string) =>
  invoke<EtatCompte>("compte_deposer_avatar", { chemin });
export const retirerAvatar = () => invoke<EtatCompte>("compte_retirer_avatar");

export const connexion = (email: string, motDePasse: string) =>
  invoke<EtatCompte>("compte_connexion", { email, motDePasse });

export const deconnexion = () => invoke<EtatCompte>("compte_deconnexion");

export const demarrerAppairage = () => invoke<DemandeAppairage>("compte_appairage_demarrer");

export const etatAppairage = (id: string) => invoke<EtatAppairage>("compte_appairage_etat", { id });

export const definirServeur = (url: string) => invoke<EtatCompte>("compte_definir_serveur", { url });

export type ResultatSynchro = {
  envoyes: number;
  recus: number;
  /** Faux quand il reste des choses à récupérer : le passage suivant continuera. */
  complet: boolean;
};

export type EtatSynchro = {
  actif: boolean;
  en_attente: number;
  dernier_passage: number | null;
};

export const synchroMaintenant = () => invoke<ResultatSynchro>("synchro_maintenant");
export const synchroEtat = () => invoke<EtatSynchro>("synchro_etat");
