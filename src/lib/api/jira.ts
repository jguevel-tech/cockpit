import { invoke } from "../coquille";

/**
 * Jira Cloud ou Server / Data Center, vu du frontend. **LE JETON N'EST JAMAIS ICI** : on n'en
 * connait que `jeton_pose`. Toute la logique (JQL, nommage de branche, git) vit cote Rust.
 */

export interface ConfigJira {
  url: string;
  jeton_pose: boolean;
  /** Pas un secret (contrairement au jeton) : requis sur Jira Cloud, vide sur Server/DC. */
  email: string;
  /** Type de ticket Jira → type de branche ; `*` = tous les autres. */
  types_branche: Record<string, string>;
}

export interface TicketJira {
  cle: string;
  resume: string;
  description: string;
  statut: string;
  /** `new`, `indeterminate` ou `done`. */
  categorie_statut: string;
  type_ticket: string;
  priorite: string;
  projet: string;
  maj_le: string;
  url: string;
}

export interface TransitionJira { id: string; nom: string; vers: string; categorie_vers: string }
export interface CommentaireJira { auteur: string; cree_le: string; corps: string }
export interface DetailTicketJira { ticket: TicketJira; commentaires: CommentaireJira[] }
export interface TypeTicketJira { id: string; nom: string }
export interface LiaisonJira { projet: string; cles: string[]; gabarit: string }
export interface DemarrageJira {
  branche: string;
  creee: boolean;
  base: string | null;
  transition: string | null;
  erreur_transition: string | null;
}

/** Une branche locale et les tickets qu'elle cite ; `cles` vide : branche sans ticket. */
export interface BrancheJira { nom: string; cles: string[] }

export const jiraConfig = () => invoke<ConfigJira>("jira_config");
export const jiraPoserConfig = (
  url: string,
  jeton: string | null,
  email: string | null,
  typesBranche: Record<string, string> | null,
) => invoke<ConfigJira>("jira_poser_config", { url, jeton, email, typesBranche });
export const jiraTester = () => invoke<string>("jira_tester");
export const jiraMesTickets = (clesProjets: string[] | null) =>
  invoke<TicketJira[]>("jira_mes_tickets", { clesProjets });
export const jiraTicket = (cle: string) => invoke<DetailTicketJira>("jira_ticket", { cle });
export const jiraTransitions = (cle: string) => invoke<TransitionJira[]>("jira_transitions", { cle });
export const jiraLiaisons = () => invoke<LiaisonJira[]>("jira_liaisons");
export const jiraLiaison = (projet: string) => invoke<LiaisonJira>("jira_liaison", { projet });
export const jiraPoserLiaison = (projet: string, cles: string, gabarit: string) =>
  invoke<LiaisonJira>("jira_poser_liaison", { projet, cles, gabarit });
export const jiraApercuBranche = (gabarit: string, cle: string, typeTicket: string, resume: string) =>
  invoke<string>("jira_apercu_branche", { gabarit, cle, typeTicket, resume });
export const jiraTransitionner = (cle: string, transitionId: string) =>
  invoke<void>("jira_transitionner", { cle, transitionId });
export const jiraCommenter = (cle: string, texte: string) => invoke<void>("jira_commenter", { cle, texte });
export const jiraSaisirTemps = (cle: string, duree: string, commentaire: string | null) =>
  invoke<void>("jira_saisir_temps", { cle, duree, commentaire });
export const jiraTypesTicket = (cleProjet: string) => invoke<TypeTicketJira[]>("jira_types_ticket", { cleProjet });
export const jiraCreerTicket = (cleProjet: string, typeId: string, resume: string, description: string | null) =>
  invoke<string>("jira_creer_ticket", { cleProjet, typeId, resume, description });
export const jiraDemarrer = (projet: string, cle: string) => invoke<DemarrageJira>("jira_demarrer", { projet, cle });
export const jiraBranches = (projet: string) => invoke<BrancheJira[]>("jira_branches", { projet });
export const jiraRenommerBranche = (projet: string, branche: string, cle: string) =>
  invoke<string>("jira_renommer_branche", { projet, branche, cle });
