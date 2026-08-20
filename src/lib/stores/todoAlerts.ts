/**
 * Producteur de notices : tâches à échéance.
 *
 * Architecture du centre de notifications (voir stores/notifications.ts) : les notices ne
 * sont jamais persistées, chaque producteur les recrée au lancement. Ici : une notice par
 * tâche en attente dont l'échéance est aujourd'hui ou dépassée. L'id stable
 * `todo-due:<id>:<date>` fait le dédoublonnage, et une tâche écartée de la cloche ne
 * revient que si son échéance change.
 */
import { getPendingTodos } from "../api/storage";
import { pushNotice, removeNotice } from "./notifications";
import { selectProject, activeTab } from "./ui";
import { daysUntil, dueLabel } from "../utils/due";
import { translate } from "../i18n";
import { signalerErreur } from "./errors";

const CHECK_MS = 30 * 60 * 1000; // le jour peut changer en cours de session

export function startTodoDueWatcher(): () => void {
  check();
  const timer = setInterval(check, CHECK_MS);
  return () => clearInterval(timer);
}

/** Ids poses lors du dernier passage : une tache terminee/supprimee retire sa notice. */
let posted = new Set<string>();

async function check() {
  let todos;
  try {
    todos = await getPendingTodos();
  } catch (e) {
      signalerErreur("todoAlerts.check", String(e));
    // Producteur de fond : pas de toast (il repassera dans 30 min), mais pas muet non plus.
    console.error("todoAlerts:", e);
    return;
  }

  const next = new Set<string>();
  for (const t of todos) {
    if (!t.due_date || daysUntil(t.due_date) > 0) continue;
    const overdue = daysUntil(t.due_date) < 0;
    const id = `todo-due:${t.id}:${t.due_date}`;
    next.add(id);
    const project = t.project;
    pushNotice({
      id,
      kind: overdue ? "warning" : "info",
      title: overdue ? translate("alerts.todoOverdue") : translate("alerts.todoToday"),
      body: `**${t.project}** — ${t.text} *(${dueLabel(t.due_date)})*`,
      createdAt: new Date().toISOString(),
      dismissible: true,
      // Alerte d'echeance : on emmene sur l'onglet des taches, pas sur l'onglet memorise
      // du projet — c'est la tache que l'utilisateur vient voir.
      action: { label: translate("alerts.seeProject"), run: () => { selectProject(project); activeTab.set("workspace"); } },
    });
  }

  // Notices devenues sans objet (tache cochee, supprimee, echeance deplacee)
  for (const id of posted) {
    if (!next.has(id)) removeNotice(id);
  }
  posted = next;
}
