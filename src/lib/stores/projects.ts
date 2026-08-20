import { get, writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { listProjects } from "../api/docker";
import { renameProject } from "../api/scanner";
import { renameProjectTab, selectProject, selectedProject } from "./ui";
import { notify } from "./toast";
import { translate } from "../i18n";
import type { Project } from "../types";
import { signalerErreur } from "./errors";

export const projects = writable<Project[]>([]);

export async function loadProjects() {
  try {
    const data = await listProjects();
    projects.set(data);
  } catch (e) {
      signalerErreur("projects.loadProjects", String(e));
    console.error("Failed to load projects:", e);
  }
}

/**
 * Renommage d'un projet — UN SEUL chemin pour tous les endroits qui l'offrent (titre de la
 * barre projet, clic droit et double-clic dans la barre laterale).
 *
 * Deux raisons d'etre ici plutot que recopie dans chaque composant :
 * - `projects.name` est UNIQUE en base. Sans controle prealable, SQLite remonte
 *   « UNIQUE constraint failed: projects.name » telle quelle jusqu'au toast — un message
 *   anglais et technique la ou l'utilisateur a juste choisi un nom deja pris.
 * - la memoire d'onglet est indexee par nom : elle doit suivre le renommage, sinon le projet
 *   revient sur Workspace au lieu de l'onglet ou on etait.
 *
 * Ne reselectionne le projet que s'il etait DEJA affiche : renommer un projet depuis la barre
 * laterale ne doit pas emmener ailleurs.
 */
export async function renommerProjet(oldName: string, newName: string): Promise<boolean> {
  const next = newName.trim();
  if (!next || next === oldName) return false;

  const pris = get(projects).some(
    (p) => p.name !== oldName && p.name.toLowerCase() === next.toLowerCase(),
  );
  if (pris) {
    notify(translate("project.nameTaken", { name: next }), "error", 4000, {
      scope: "projet.renommage",
      report: false,
    });
    return false;
  }

  try {
    await renameProject(oldName, next);
  } catch (e) {
    notify(String(e), "error", 4000, { scope: "projet.renommage" });
    return false;
  }
  await loadProjects();
  const etaitAffiche = get(selectedProject) === oldName;
  renameProjectTab(oldName, next);
  if (etaitAffiche) selectProject(next);
  return true;
}

// Listen for status updates from backend
listen("status_update", async () => {
  await loadProjects();
});
