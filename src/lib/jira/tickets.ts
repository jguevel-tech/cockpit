import type { BrancheJira, LiaisonJira, TicketJira } from "../api/jira";

/**
 * Le rangement des tickets, sans dependance : teste par `scripts/tests/jira-tickets.test.mjs`.
 */

/** Jira ne renvoie pas les termines ; on les ecarte quand meme si l'un passe. */
export function grouperParCategorie(tickets: TicketJira[]): { enCours: TicketJira[]; aFaire: TicketJira[] } {
  return {
    enCours: tickets.filter((t) => t.categorie_statut === "indeterminate"),
    aFaire: tickets.filter((t) => t.categorie_statut !== "indeterminate" && t.categorie_statut !== "done"),
  };
}

/** Le premier projet Cockpit dont les cles contiennent celle du projet Jira du ticket. */
export function liaisonDuTicket(ticket: TicketJira, liaisons: LiaisonJira[]): LiaisonJira | null {
  return liaisons.find((l) => l.cles.includes(ticket.projet)) ?? null;
}

/**
 * `""` : tous mes tickets (`null`). Un projet : ses cles — **LISTE VIDE S'IL N'EN A PAS**, que le
 * backend traduit par « aucun ticket », jamais par « tous ».
 */
export function clesDuFiltre(filtre: string, liaisons: LiaisonJira[]): string[] | null {
  if (!filtre) return null;
  return liaisons.find((l) => l.projet === filtre)?.cles ?? [];
}

export interface BrancheOrpheline { projet: string; nom: string }

/**
 * Branches par ticket et branches sans ticket. `parProjet` ne porte que les projets dont les
 * branches ont pu etre lues : un projet absent n'est ni « sans branche » ni « tout va bien ».
 */
export function rapprocher(parProjet: Record<string, BrancheJira[]>): {
  parTicket: Record<string, string[]>;
  orphelines: BrancheOrpheline[];
} {
  const parTicket: Record<string, string[]> = {};
  const orphelines: BrancheOrpheline[] = [];
  for (const [projet, branches] of Object.entries(parProjet)) {
    for (const b of branches) {
      if (b.cles.length === 0) orphelines.push({ projet, nom: b.nom });
      for (const cle of b.cles) (parTicket[cle] ??= []).push(b.nom);
    }
  }
  return { parTicket, orphelines };
}

/** Un resume de depart tire du dernier segment : `feature/refonte-login` → « Refonte login ». */
export function resumeDeBranche(nom: string): string {
  const mots = (nom.split("/").pop() ?? "").replace(/[-_]+/g, " ").trim();
  return mots.charAt(0).toUpperCase() + mots.slice(1);
}
