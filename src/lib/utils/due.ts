/**
 * Échéances de tâches : une due_date est une DATE DE CALENDRIER locale ("2026-08-20"),
 * jamais un instant UTC — comparer via Date.parse ferait basculer l'échéance d'un jour
 * selon le fuseau. Tout se calcule à minuit local.
 */

export function daysUntil(due: string): number {
  const [y, m, d] = due.split("-").map(Number);
  const dueDate = new Date(y, m - 1, d);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  return Math.round((dueDate.getTime() - today.getTime()) / 86_400_000);
}

/** Libellé court pour un badge : "aujourd'hui", "demain", "hier", "en retard de 3 j", "20/08". */
export function dueLabel(due: string): string {
  const n = daysUntil(due);
  if (n < -1) return `en retard de ${-n} j`;
  if (n === -1) return "hier";
  if (n === 0) return "aujourd'hui";
  if (n === 1) return "demain";
  const [, m, d] = due.split("-");
  return `${d}/${m}`;
}

export type DueUrgency = "overdue" | "today" | "later";

export function dueUrgency(due: string): DueUrgency {
  const n = daysUntil(due);
  return n < 0 ? "overdue" : n === 0 ? "today" : "later";
}
