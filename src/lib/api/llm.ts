import { invoke } from "@tauri-apps/api/core";

/**
 * Les fournisseurs d'IA, vus du frontend.
 *
 * **RIEN ICI NE NOMME UN PRODUIT.** Le catalogue vit cote Rust (`src-tauri/src/llm/`) et dit ce
 * que chacun sait faire ; l'interface n'affiche que ce qui existe. C'est ce qui permet d'en
 * declarer un nouveau sans toucher a un seul composant — et ce qui evite un bouton qui promet
 * ce que le fournisseur ne sait pas faire.
 */

/** Ce qu'un fournisseur sait faire, sur cette machine. */
export interface CapacitesLlm {
  id: string;
  nom: string;
  /** Un caractere pour le designer. Pas une image : voir le catalogue cote Rust. */
  symbole: string;
  couleur: string;
  /** Son CLI est installe ici. */
  cli: boolean;
  /** Il a un CLI, tout court (faux pour un fournisseur d'API pure). */
  a_un_cli: boolean;
  conversations: boolean;
  abonnement: boolean;
  texte: boolean;
  transcription: boolean;
  /** Ses agents s'installent au format de plugins de Claude Code. */
  plugins: boolean;
  cle_requise: boolean;
  /** La cle est posee. **Jamais la cle elle-meme** : elle ne remonte pas jusqu'ici. */
  cle_posee: boolean;
  prefere: boolean;
}

/** Une conversation passee, chez le fournisseur choisi. */
export interface ConversationLlm {
  id: string;
  label: string;
  updated_at: number;
  renamed: boolean;
}

/** Les commandes de terminal du fournisseur : elles viennent de LUI. */
export interface CommandesAgent {
  neuve: string;
  reprise: string | null;
}

export interface EtatAbonnement {
  fournisseur: string;
  nom: string;
  /** Ce fournisseur parle-t-il d'abonnement ? Sinon il n'y a rien a connecter. */
  gere_abonnement: boolean;
  cli_installe: boolean;
  cli_version: string | null;
  connexion_guidee: boolean;
  connecte: boolean;
  formule: string | null;
  palier: string | null;
  expire_le: number | null;
  /**
   * Pourquoi l'etat n'a pas pu etre determine. « Non connecte » et « on n'a pas su regarder »
   * sont deux choses differentes : elles affichaient le meme badge, et on relancait une
   * connexion qui ne changeait rien.
   */
  probleme: string | null;
}

export const catalogueLlm = () => invoke<CapacitesLlm[]>("llm_catalogue");
export const choisirLlm = (id: string) => invoke("llm_choisir", { id });
export const poserCleLlm = (id: string, cle: string) => invoke("llm_poser_cle", { id, cle });

export const conversationsLlm = (projectPath: string) =>
  invoke<ConversationLlm[]>("llm_conversations", { projectPath });
export const renommerConversationLlm = (conversationId: string, nom: string) =>
  invoke("llm_renommer_conversation", { conversationId, nom });
export const commandesLlm = (conversationId?: string) =>
  invoke<CommandesAgent>("llm_commandes", { conversationId: conversationId ?? null });

/** Qui transcrira et qui redigera le prochain compte rendu. La regle vit cote Rust. */
export interface AffectationsReunion {
  transcription: string | null;
  redaction: string | null;
}
export const reunionsLlm = () => invoke<AffectationsReunion>("llm_reunions");

export const abonnementLlm = (id?: string) =>
  invoke<EtatAbonnement>("llm_abonnement", { id: id ?? null });
export const demarrerConnexionLlm = (id?: string) =>
  invoke("llm_connexion_demarrer", { id: id ?? null });
export const entrerConnexionLlm = (data: string) => invoke("llm_connexion_entrer", { data });
export const annulerConnexionLlm = () => invoke("llm_connexion_annuler");

/** Les evenements de la connexion guidee. Memes noms que cote Rust. */
export const EVENEMENT_CONNEXION_SORTIE = "llm_connexion_sortie";
export const EVENEMENT_CONNEXION_FIN = "llm_connexion_fin";
