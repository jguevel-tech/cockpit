/**
 * Échéances de tâches : une due_date est une DATE DE CALENDRIER locale ("2026-08-20"),
 * jamais un instant UTC — comparer via Date.parse ferait basculer l'échéance d'un jour
 * selon le fuseau. Tout se calcule à minuit local.
 */
import { translate } from "../i18n";

export function daysUntil(due: string): number {
  const [y, m, d] = due.split("-").map(Number);
  const dueDate = new Date(y, m - 1, d);
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  return Math.round((dueDate.getTime() - today.getTime()) / 86_400_000);
}

/**
 * Libellé court pour un badge : "aujourd'hui", "demain", "hier", "en retard de 3 j", "20/08".
 *
 * Les quatre libellés étaient écrits en français DANS ce fichier : ils restaient français en
 * anglais, sur l'écran le plus lu de l'application. Et l'ordre jour/mois vit lui aussi dans le
 * catalogue — un anglophone lit 09/10 pour le 9 octobre, l'inverse de ce que dit notre format.
 *
 * Le retard est toujours de deux jours au moins ici (`n < -1`), donc un seul libellé suffit :
 * "hier" a sa propre clé.
 */
export function dueLabel(due: string): string {
  const n = daysUntil(due);
  if (n < -1) return translate("due.overdue", { n: -n });
  if (n === -1) return translate("due.yesterday");
  if (n === 0) return translate("due.today");
  if (n === 1) return translate("due.tomorrow");
  const [, m, d] = due.split("-");
  return translate("due.date", { d, m });
}

export type DueUrgency = "overdue" | "today" | "later";

export function dueUrgency(due: string): DueUrgency {
  const n = daysUntil(due);
  return n < 0 ? "overdue" : n === 0 ? "today" : "later";
}
