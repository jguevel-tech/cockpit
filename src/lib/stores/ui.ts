import { writable } from "svelte/store";
import { setWebviewZoom } from "../api/system";

/// Vue top-niveau unique : une seule source de verite pour la navigation.
/// Ajouter une vue = ajouter une entree ici + un case dans MainPanel.
export type ActiveView = "dashboard" | "project" | "settings" | "system";

export const activeView = writable<ActiveView>("dashboard");
export const selectedProject = writable<string | null>(null);
export const activeTab = writable<"workspace" | "docker" | "terminal" | "files" | "git" | "sitemap" | "settings" | "plugins">("workspace");
// Session terminal a activer a l'arrivee sur l'onglet Terminal (navigation depuis le dashboard)
export const pendingTerminalId = writable<number | null>(null);
// Sous-vue active du tableau de bord
export const dashboardView = writable<"tasks" | "monitoring" | "terminals" | "containers">("tasks");
// Le theme a demenage dans stores/appearance.ts : ce n'est plus un booleen sombre/clair mais
// une palette parmi plusieurs, accompagnee d'un accent personnalisable et d'une image de fond.
// Consommer `theme` (identifiant de palette) ou `themeBase` ("dark" | "light") depuis la.

// --- Zoom global ---

/// Taille de police des terminaux xterm, en px CSS. Sert aussi de base aux paliers de
/// zoom (voir ZOOM_LEVELS) : les deux DOIVENT rester coherents.
export const TERMINAL_FONT_SIZE = 13;

/// Tailles de police terminal visees par les paliers de zoom, en px ENTIERS.
///
/// Pourquoi pas des paliers ronds (1.1, 1.25, 1.4...) : le zoom webview multiplie tout,
/// donc un facteur quelconque donne une police en pixels fractionnaires (13 x 1.1 = 14.3 px)
/// que le rasteriseur doit lisser -> texte mou. L'effet est d'autant plus visible que les
/// glyphes sont petits : constate a 110 %, invisible a 150 % et 200 % (13 x 2 = 26 px pile).
/// En visant des tailles entieres, chaque palier tombe sur des pixels pleins.
///
/// L'UI (racine 14 px) ne peut pas etre exacte en meme temps : pour que 13z ET 14z soient
/// entiers il faut z entier, donc seuls 100 % et 200 % le permettraient. On privilegie le
/// terminal (lecture continue) ; la racine atterrit a ~0,2 px d'un entier, sous le seuil visible.
const TERMINAL_FONT_STEPS = [10, 11, 12, 13, 14, 15, 16, 18, 20, 22, 26];

/// Paliers de zoom. Bornes alignees sur ZOOM_MIN/ZOOM_MAX (src-tauri/src/lib.rs).
export const ZOOM_LEVELS = TERMINAL_FONT_STEPS.map((px) => px / TERMINAL_FONT_SIZE);
const ZOOM_DEFAULT = 1;

export const zoom = writable<number>(ZOOM_DEFAULT);

/// Palier valide le plus proche : protege des valeurs heritees d'un localStorage
/// ecrit par une version aux paliers differents.
function nearestLevel(value: number): number {
  return ZOOM_LEVELS.reduce((best, l) =>
    Math.abs(l - value) < Math.abs(best - value) ? l : best
  );
}

if (typeof window !== "undefined") {
  const saved = parseFloat(localStorage.getItem("cockpit-zoom") ?? "");
  if (Number.isFinite(saved)) zoom.set(nearestLevel(saved));
}

zoom.subscribe((z) => {
  if (typeof window === "undefined") return;
  localStorage.setItem("cockpit-zoom", String(z));
  // Le re-fit des terminaux est assure par le ResizeObserver de TerminalTab :
  // zoomer change les dimensions en px CSS du conteneur.
  setWebviewZoom(z).catch((e) => {
    // Pas de notify() ici : le store est importe par des modules charges avant
    // le montage de Toast, et un echec de zoom n'empeche pas d'utiliser l'app.
    console.error("set_webview_zoom", e);
  });
});

function stepZoom(direction: 1 | -1) {
  zoom.update((z) => {
    // nearestLevel renvoie un element de ZOOM_LEVELS : indexOf le retrouve a l'identique
    // malgre les flottants (14/13 n'a pas de representation exacte).
    const i = ZOOM_LEVELS.indexOf(nearestLevel(z));
    return ZOOM_LEVELS[Math.min(ZOOM_LEVELS.length - 1, Math.max(0, i + direction))];
  });
}

export const zoomIn = () => stepZoom(1);
export const zoomOut = () => stepZoom(-1);
export const zoomReset = () => zoom.set(ZOOM_DEFAULT);

export function openView(view: Exclude<ActiveView, "project">) {
  selectedProject.set(null);
  activeView.set(view);
}

export function selectProject(name: string | null) {
  selectedProject.set(name);
  activeView.set(name ? "project" : "dashboard");
  activeTab.set("workspace");
}

export const openSettings = () => openView("settings");
export const openSystem = () => openView("system");
export const goHome = () => openView("dashboard");
