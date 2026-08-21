import { invoke } from "@tauri-apps/api/core";

/** L'etat du compte tel que le backend le voit. */
export type EtatCompte = {
  connecte: boolean;
  email: string | null;
  nom: string | null;
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

export const inscription = (email: string, motDePasse: string) =>
  invoke<EtatCompte>("compte_inscription", { email, motDePasse });

export const connexion = (email: string, motDePasse: string) =>
  invoke<EtatCompte>("compte_connexion", { email, motDePasse });

export const deconnexion = () => invoke<EtatCompte>("compte_deconnexion");

export const demarrerAppairage = () => invoke<DemandeAppairage>("compte_appairage_demarrer");

export const etatAppairage = (id: string) => invoke<EtatAppairage>("compte_appairage_etat", { id });

export const definirServeur = (url: string) => invoke<EtatCompte>("compte_definir_serveur", { url });
