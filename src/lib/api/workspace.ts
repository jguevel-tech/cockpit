import { invoke } from "@tauri-apps/api/core";
import type { TerminalInfo, DirEntry, FileContent, GitStatus, FileDiff, BranchInfo, ClaudeSession, HistoryEntry, GotoDefinitionResult } from "../types";

// Terminaux integres
export const createTerminal = (project: string, cwd: string, cols: number, rows: number, initCommand?: string) =>
  invoke<number>("create_terminal", { project, cwd, cols, rows, initCommand: initCommand ?? null });
export const writeTerminal = (id: number, data: string) => invoke("write_terminal", { id, data });
export const resizeTerminal = (id: number, cols: number, rows: number) =>
  invoke("resize_terminal", { id, cols, rows });
export const closeTerminal = (id: number) => invoke("close_terminal", { id });
// Presse-papier systeme (copie OSC 52 depuis tmux)
export const setClipboard = (text: string) => invoke("set_clipboard", { text });
export const getClipboard = () => invoke<string>("get_clipboard");
// Copie la selection copy-mode tmux du terminal (clic droit > Copier)
export const terminalCopySelection = (id: number) => invoke("terminal_copy_selection", { id });
export const attachTerminal = (id: number, cols: number, rows: number) =>
  invoke<string>("attach_terminal", { id, cols, rows });
export const detachTerminal = (id: number) => invoke("detach_terminal", { id });
export const renameTerminal = (id: number, name: string) => invoke("rename_terminal", { id, name });
export const listTerminals = (project: string) => invoke<TerminalInfo[]>("list_terminals", { project });
export const listAllTerminals = () => invoke<TerminalInfo[]>("list_all_terminals");
export const listClaudeSessions = (projectPath: string) =>
  invoke<ClaudeSession[]>("list_claude_sessions", { projectPath });
export const renameClaudeSession = (sessionId: string, name: string) =>
  invoke("rename_claude_session", { sessionId, name });
// Connexion Claude Code (abonnement)
export interface ClaudeAuthStatus {
  cli_installed: boolean;
  cli_version: string | null;
  logged_in: boolean;
  subscription_type: string | null;
  rate_limit_tier: string | null;
  expires_at: number | null;
}
export const claudeAuthStatus = () => invoke<ClaudeAuthStatus>("claude_auth_status");
export const startClaudeLogin = () => invoke("start_claude_login");
export const claudeLoginInput = (data: string) => invoke("claude_login_input", { data });
export const cancelClaudeLogin = () => invoke("cancel_claude_login");
export const openUrl = (url: string) => invoke("open_url", { url });

export const recordCommand = (project: string, command: string) =>
  invoke("record_command", { project, command });
export const terminalAltScreen = (id: number) => invoke<boolean>("terminal_alt_screen", { id });
export const searchCommandHistory = (query: string, limit?: number) =>
  invoke<HistoryEntry[]>("search_command_history", { query, limit: limit ?? null });

// Explorateur de fichiers
export const listProjectDir = (projectPath: string, relPath: string) =>
  invoke<DirEntry[]>("list_project_dir", { projectPath, relPath });
export const readProjectFile = (projectPath: string, relPath: string) =>
  invoke<FileContent>("read_project_file", { projectPath, relPath });
export const writeProjectFile = (projectPath: string, relPath: string, content: string) =>
  invoke("write_project_file", { projectPath, relPath, content });
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
export const gitBranches = (projectPath: string) => invoke<BranchInfo[]>("git_branches", { projectPath });
export const gitCheckoutBranch = (projectPath: string, name: string) =>
  invoke("git_checkout_branch", { projectPath, name });
export const gitCreateBranch = (projectPath: string, name: string) =>
  invoke("git_create_branch", { projectPath, name });
export const gitDeleteBranch = (projectPath: string, name: string, force: boolean) =>
  invoke("git_delete_branch", { projectPath, name, force });
