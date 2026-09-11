import { invoke } from "../coquille";

/// Une fenetre de limitation : la part consommee, et l'instant ou elle repart a zero.
/// `cle` vaut "session" ou "semaine" : le libelle affiche vient du catalogue, pas du backend.
export interface Fenetre {
  cle: string;
  pourcentage: number;
  /// Epoch SECONDES. Absent quand le fournisseur ne l'annonce pas : on montre alors la part
  /// consommee sans promettre une heure qu'on ne connait pas.
  remise_a_zero: number | null;
}

export interface EtatConsommation {
  fournisseur: string;
  nom: string;
  /// Le fournisseur sait-il seulement parler de consommation ? Sinon l'indicateur n'existe pas.
  gere: boolean;
  fenetres: Fenetre[];
  /// Pourquoi la mesure a echoue. « Rien a montrer » et « on n'a pas su regarder » ne
  /// s'affichent pas pareil.
  probleme: string | null;
}

/// La consommation du fournisseur choisi, ou de celui qu'on nomme.
export const lireConsommation = (id?: string) =>
  invoke<EtatConsommation>("llm_consommation", id ? { id } : {});
