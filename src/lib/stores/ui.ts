import { writable } from "svelte/store";
import { setWebviewZoom } from "../api/system";

const READING_KEY = "cockpit-notes-reading";

/// Vue top-niveau unique : une seule source de verite pour la navigation.
/// Ajouter une vue = ajouter une entree ici + un case dans MainPanel.
export type ActiveView = "dashboard" | "project" | "settings" | "system" | "docs";

export const activeView = writable<ActiveView>("dashboard");
export const selectedProject = writable<string | null>(null);

/// Onglets de la vue projet. Ajouter un onglet = une entree ici + une entree dans la map
/// `tabs` de ProjectDetail.svelte.
export type ProjectTab = "workspace" | "docker" | "terminal" | "files" | "git" | "settings" | "plugins";

/// Onglet d'arrivee sur un projet dont on ne sait rien (jamais visite, ou fraichement
/// cree : sans compose ni depot git, Docker et Git n'auraient rien a montrer).
export const DEFAULT_TAB: ProjectTab = "workspace";

export const activeTab = writable<ProjectTab>(DEFAULT_TAB);
// Session terminal a activer a l'arrivee sur l'onglet Terminal (navigation depuis le dashboard)
export const pendingTerminalId = writable<number | null>(null);
/** Fichier a ouvrir a l'arrivee sur l'onglet Fichiers (palette Ctrl+K), meme mecanique
 *  que pendingTerminalId : pose puis consomme et remis a null par FilesTab. */
export const pendingFilePath = writable<string | null>(null);
/** Commande a lancer dans un NOUVEAU terminal du projet (bouton ▶ Cmd, shell d'un
 *  conteneur, palette Ctrl+K). Meme mecanique que pendingTerminalId : posee ici,
 *  consommee par TerminalTab au montage ET a chaud, toujours remise a null.
 *
 *  Pourquoi passer par un magasin plutot que d'appeler `create_terminal` sur place :
 *  seul l'onglet Terminal connait la taille de son conteneur, et une TUI lancee a la
 *  creation (k9s, htop, top) se dessine a la taille du PTY. Creer la session a une
 *  taille arbitraire la laisse dans un petit carre — voir `honorerCommande`. */
// `dossier` permet d'ouvrir le terminal AILLEURS que dans le dossier du projet — c'est ce qui
// sert aux worktrees git. Absent = le dossier du projet, comme avant.
export const pendingTerminalCommand = writable<{
  project: string;
  command: string;
  dossier?: string;
} | null>(null);
// Sous-vue active du tableau de bord
export const dashboardView = writable<"tasks" | "monitoring" | "terminals" | "containers">("tasks");

// --- Mode lecture de l'onglet Workspace ---

/// Replie d'un coup l'arborescence des notes (gauche) ET la colonne des taches (droite) pour
/// donner toute la largeur au compte rendu. Global et non par projet : c'est une preference de
/// confort de lecture, elle n'a pas de raison de changer d'un projet a l'autre.
export const readingMode = writable<boolean>(
  typeof window !== "undefined" && localStorage.getItem(READING_KEY) === "1",
);

readingMode.subscribe((on) => {
  if (typeof window === "undefined") return;
  localStorage.setItem(READING_KEY, on ? "1" : "0");
});

export const toggleReadingMode = () => readingMode.update((on) => !on);

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

/// Taille de police terminal du zoom PAR DEFAUT — celle sur laquelle les pourcentages
/// affiches sont comptes, donc celle qui s'affiche « 100 % ».
///
/// 15 px et non 13 : le mainteneur trouve ce rendu plus agreable et l'a demande comme reference
/// (« le 115 % devrait etre la version 100 % »). Ce n'est pas qu'un defaut de demarrage — le
/// bouton du milieu, qui remet le zoom a sa valeur de reference, y revient aussi.
///
/// Les FACTEURS ne changent pas : ils restent tires de tailles de police entieres, sinon le
/// texte devient mou (voir TERMINAL_FONT_STEPS). Seul le point de comptage se deplace, donc
/// les paliers s'affichent desormais 67, 73, 80, 87, 93, **100**, 107, 120, 133, 147, 173 %.
/// Le zoom REEL de quelqu'un qui en avait choisi un ne bouge pas : c'est son etiquette qui
/// change.
const TERMINAL_FONT_DEFAULT = 15;
const ZOOM_DEFAULT = TERMINAL_FONT_DEFAULT / TERMINAL_FONT_SIZE;

/// Pourcentage a afficher pour un palier, compte depuis le zoom par defaut.
export function zoomPourcent(niveau: number): number {
  return Math.round((niveau / ZOOM_DEFAULT) * 100);
}

export const zoom = writable<number>(ZOOM_DEFAULT);

/// Palier valide le plus proche : protege des valeurs heritees d'un localStorage
/// ecrit par une version aux paliers differents.
function nearestLevel(value: number): number {
  return ZOOM_LEVELS.reduce((best, l) =>
    Math.abs(l - value) < Math.abs(best - value) ? l : best
  );
}

if (typeof window !== "undefined") {
  const brut = localStorage.getItem("cockpit-zoom");
  // Reprise du changement de reference : l'ancien defaut s'ecrivait exactement « 1 », et le
  // magasin enregistre des l'initialisation — donc TOUT LE MONDE a cette valeur, y compris
  // ceux qui n'ont jamais touche au zoom. On les emmene sur le nouveau defaut, sinon le
  // changement ne se verrait que sur une installation neuve.
  //
  // Consequence assumee : quelqu'un qui avait CHOISI l'ancien 100 % est deplace lui aussi. Il
  // n'y a pas moyen de distinguer les deux, et deux clics suffisent a revenir.
  const saved = parseFloat(brut ?? "");
  if (brut === "1") zoom.set(ZOOM_DEFAULT);
  else if (Number.isFinite(saved)) zoom.set(nearestLevel(saved));
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

// --- Memoire de l'onglet par projet ---

/// Chaque projet se souvient de l'onglet ou on l'a laisse : partir voir autre chose puis
/// revenir le retrouve tel qu'on l'a quitte, au lieu de repartir de Workspace a chaque
/// aller-retour. Ce n'est PAS un onglet global traine de projet en projet : deux projets
/// laisses sur des onglets differents gardent chacun le sien.
///
/// En memoire seulement, volontairement : la demande porte sur les allers-retours d'une
/// session de travail. Persister obligerait a purger les projets disparus et a decider ce
/// que vaut un onglet vieux de trois jours, pour un gain nul.
const tabByProject = new Map<string, ProjectTab>();

/// Projet a crediter quand activeTab change. Passer par le store plutot que par les
/// appelants garde la memoire juste quel que soit le chemin emprunte (clic sur l'onglet,
/// palette Ctrl+K, raccourci du tableau de bord) — rien a penser en ajoutant un appelant.
let rememberFor: string | null = null;
selectedProject.subscribe((name) => { rememberFor = name; });

/// Vrai quand l'onglet affiche vient de la memoire ci-dessus, faux quand l'utilisateur l'a
/// demande. TerminalTab le consomme au montage : un onglet Terminal RESTAURE ne doit pas
/// ouvrir un shell d'office, sinon parcourir trois projets laisses sur cet onglet en
/// ouvre trois que personne n'a demandes.
let tabRestored = false;

activeTab.subscribe((tab) => {
  if (rememberFor) tabByProject.set(rememberFor, tab);
  // Un set explicite (quel qu'en soit l'appelant) est une demande de l'utilisateur.
  // selectProject repose le drapeau APRES son propre set.
  tabRestored = false;
});

/// A appeler quand un projet disparait, sinon un projet recree sous le meme nom
/// ressortirait l'onglet du precedent.
export function forgetProjectTab(name: string) {
  tabByProject.delete(name);
}

/// Un projet renomme est le MEME projet : il emporte son onglet. Sans ce transfert le
/// renommage renverrait l'utilisateur sur Workspace, alors qu'il n'a rien demande de tel.
export function renameProjectTab(from: string, to: string) {
  const tab = tabByProject.get(from);
  tabByProject.delete(from);
  if (tab) tabByProject.set(to, tab);
}

/// Lit et remet a zero le drapeau de restauration (voir tabRestored).
export function consumeTabRestored(): boolean {
  const restored = tabRestored;
  tabRestored = false;
  return restored;
}

export function selectProject(name: string | null) {
  selectedProject.set(name);
  activeView.set(name ? "project" : "dashboard");
  if (!name) return;
  const remembered = tabByProject.get(name);
  const tab = remembered ?? DEFAULT_TAB;
  activeTab.set(tab);
  // Les deux lignes qui suivent sont ecrites APRES le set, et pas laissees a la
  // souscription : un writable ne notifie pas quand la valeur ne change pas (deux projets
  // sur le meme onglet), et c'est justement ce cas qui multipliait les sessions.
  tabByProject.set(name, tab);
  tabRestored = remembered !== undefined;
}

export const openSettings = () => openView("settings");
export const openDocs = () => openView("docs");
export const openSystem = () => openView("system");
export const goHome = () => openView("dashboard");
