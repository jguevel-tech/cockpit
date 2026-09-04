import { invoke } from "@tauri-apps/api/core";
import type { TerminalInfo, TerminalSearchResult, DirEntry, FileContent, FileStat, GitStatus, FileDiff, BranchInfo, CommitInfo, HistoryEntry, GotoDefinitionResult, SearchResults, Worktree } from "../types";

// Terminaux integres
export const createTerminal = (project: string, cwd: string, cols: number, rows: number, initCommand?: string) =>
  invoke<number>("create_terminal", { project, cwd, cols, rows, initCommand: initCommand ?? null });
export const writeTerminal = (id: number, data: string) => invoke("write_terminal", { id, data });
export const resizeTerminal = (id: number, cols: number, rows: number) =>
  invoke("resize_terminal", { id, cols, rows });
export const closeTerminal = (id: number) => invoke("close_terminal", { id });
// Presse-papier systeme (menu clic droit, clic molette, OSC 52 d'un programme)
export const setClipboard = (text: string) => invoke("set_clipboard", { text });
export const getClipboard = () => invoke<string>("get_clipboard");
// Branche la sortie du terminal sur l'interface. Sans effet s'il l'est deja : re-brancher
// redemanderait un redessin complet, donc un clignotement a chaque changement d'onglet.
export const attachTerminal = (id: number, cols: number, rows: number) =>
  invoke("attach_terminal", { id, cols, rows });
export const renameTerminal = (id: number, name: string) => invoke("rename_terminal", { id, name });
// Photographie les terminaux ouverts, pour qu'ils reviennent comme on les a quittes apres
// une extinction du poste. A appeler sur un GESTE (quitter la vue des terminaux), jamais sur
// un minuteur : le cout se paie par terminal, et le backend refuse de recommencer avant une
// minute. La fermeture de la fenetre declenche la meme chose cote Rust.
export const saveTerminalScreens = () => invoke("save_terminal_screens");
export const listTerminals = (project: string) => invoke<TerminalInfo[]>("list_terminals", { project });
export const listAllTerminals = () => invoke<TerminalInfo[]>("list_all_terminals");
export const openUrl = (url: string) => invoke("open_url", { url });

export const recordCommand = (project: string, command: string) =>
  invoke("record_command", { project, command });
// Recherche dans le terminal, historique compris. Le serveur n'a pas d'ecran a peindre :
// il rend OU se trouve l'occurrence, c'est le terminal qui defile et surligne.
export const terminalSearch = (id: number, action: "start" | "next" | "prev" | "cancel", query = "") =>
  invoke<TerminalSearchResult>("terminal_search", { id, action, query });
export const searchCommandHistory = (query: string, limit?: number) =>
  invoke<HistoryEntry[]>("search_command_history", { query, limit: limit ?? null });

// Explorateur de fichiers
export const listProjectDir = (projectPath: string, relPath: string) =>
  invoke<DirEntry[]>("list_project_dir", { projectPath, relPath });
export const readProjectFile = (projectPath: string, relPath: string) =>
  invoke<FileContent>("read_project_file", { projectPath, relPath });
// Etat disque du fichier affiche : null s'il n'existe plus (suivi des
// modifications exterieures, cf. FilesTab). Un stat, pas une relecture.
export const statProjectFile = (projectPath: string, relPath: string) =>
  invoke<FileStat | null>("stat_project_file", { projectPath, relPath });
export const writeProjectFile = (projectPath: string, relPath: string, content: string) =>
  invoke("write_project_file", { projectPath, relPath, content });
// Recherche globale : noms de dossiers/fichiers + contenu, gitignore-aware
export const searchProject = (projectPath: string, query: string) =>
  invoke<SearchResults>("search_project", { projectPath, query });
// Apercu d'images (data URL, 10 Mo max)
export const readProjectImage = (projectPath: string, relPath: string) =>
  invoke<string>("read_project_image", { projectPath, relPath });
// Gestion de fichiers (racine verrouillee ; suppression = corbeille systeme)
export const createProjectFile = (projectPath: string, relDir: string, name: string) =>
  invoke<string>("create_project_file", { projectPath, relDir, name });
export const createProjectDir = (projectPath: string, relDir: string, name: string) =>
  invoke<string>("create_project_dir", { projectPath, relDir, name });
export const renameProjectEntry = (projectPath: string, relPath: string, newName: string) =>
  invoke<string>("rename_project_entry", { projectPath, relPath, newName });
export const trashProjectEntry = (projectPath: string, relPath: string) =>
  invoke("trash_project_entry", { projectPath, relPath });
// Aller a la definition (LSP si serveur dispo, sinon recherche de declarations)
export const gotoDefinition = (
  projectPath: string, lang: string, relPath: string, content: string,
  line: number, character: number, symbol: string,
) => invoke<GotoDefinitionResult>("goto_definition", { projectPath, lang, relPath, content, line, character, symbol });

// Git
export const gitStatus = (projectPath: string) => invoke<GitStatus>("git_status", { projectPath });
export const gitDiffFile = (projectPath: string, path: string, untracked: boolean) =>
  invoke<FileDiff>("git_diff_file", { projectPath, path, untracked });
export const gitStage = (projectPath: string, path: string) => invoke("git_stage", { projectPath, path });
export const gitUnstage = (projectPath: string, path: string) => invoke("git_unstage", { projectPath, path });
export const gitStageAll = (projectPath: string) => invoke("git_stage_all", { projectPath });
export const gitUnstageAll = (projectPath: string) => invoke("git_unstage_all", { projectPath });
export const gitCommit = (projectPath: string, message: string) => invoke("git_commit", { projectPath, message });
export const gitPush = (projectPath: string, setUpstream: boolean) =>
  invoke<string>("git_push", { projectPath, setUpstream });
export const gitPull = (projectPath: string) => invoke<string>("git_pull", { projectPath });
export const gitLog = (projectPath: string, limit = 100) =>
  invoke<CommitInfo[]>("git_log", { projectPath, limit });
export const gitCommitDiff = (projectPath: string, hash: string) =>
  invoke<FileDiff[]>("git_commit_diff", { projectPath, hash });
export const gitBranches = (projectPath: string) => invoke<BranchInfo[]>("git_branches", { projectPath });
export const gitWorktrees = (projectPath: string) => invoke<Worktree[]>("git_worktrees", { projectPath });
export const gitWorktreeAdd = (projectPath: string, branche: string, creer: boolean) =>
  invoke<string>("git_worktree_add", { projectPath, branche, creer });
export const gitWorktreeRemove = (projectPath: string, chemin: string, force: boolean) =>
  invoke<void>("git_worktree_remove", { projectPath, chemin, force });
export const gitCheckoutBranch = (projectPath: string, name: string) =>
  invoke("git_checkout_branch", { projectPath, name });
export const gitCreateBranch = (projectPath: string, name: string) =>
  invoke("git_create_branch", { projectPath, name });
export const gitDeleteBranch = (projectPath: string, name: string, force: boolean) =>
  invoke("git_delete_branch", { projectPath, name, force });

/**
 * La langue imposee au demarrage par l'environnement, ou `null`.
 *
 * Elle n'existe que pour le harnais de captures : le site vitrine a besoin des memes ecrans en
 * francais et en anglais, et la langue vit dans le `localStorage` — donc impossible a poser
 * avant le premier rendu. Piloter les menus pour la changer aurait rendu les captures
 * dependantes de la position d'une entree de menu.
 */
export async function langueImposee(): Promise<"fr" | "en" | null> {
  const valeur = await invoke<string | null>("langue_imposee");

  return valeur === "fr" || valeur === "en" ? valeur : null;
}
