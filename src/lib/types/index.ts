export type ProjectState = "stopped" | "starting" | "running" | "stopping" | "error";

export interface ContainerStatus {
  name: string;
  service: string;
  status: string;
  health: string;
  ports: string;
}

export interface Project {
  name: string;
  path: string;
  description: string;
  depends_on: string[];
  depended_by: string[];
  state: ProjectState;
  containers: ContainerStatus[];
  error?: string;
  /** Un fichier compose exploitable existe-t-il ? Sinon start/stop ne peuvent pas aboutir. */
  has_compose: boolean;
  folder_id: number | null;
}

export interface DbProject {
  id: number;
  name: string;
  path: string;
  compose_file: string;
  description: string;
  depends_on: string[];
  position: number;
  folder_id: number | null;
  created_at: string;
}

export interface ProjectFolder {
  id: number;
  name: string;
  position: number;
  /** Dossier parent, ou null au premier niveau. L'imbrication n'a pas de limite. */
  parent_id: number | null;
}

export interface Todo {
  id: number;
  project: string;
  text: string;
  done: boolean;
  position: number;
  created_at: string;
  /** Échéance optionnelle, date ISO "2026-08-20" (null = sans échéance) */
  due_date: string | null;
}

export interface Note {
  id: number;
  project: string;
  content: string;
  created_at: string;
  updated_at: string;
}

export interface NoteFolder {
  id: number;
  project: string;
  parent_id: number | null;
  name: string;
  position: number;
}

export interface NoteFile {
  id: number;
  project: string;
  folder_id: number | null;
  name: string;
  content: string;
  position: number;
  updated_at: string;
}

export interface NoteTree {
  folders: NoteFolder[];
  files: NoteFile[];
}

export interface Url {
  id: number;
  project: string;
  label: string;
  url: string;
  position: number;
}

/** Statut up/down d'un lien rapide (HEAD HTTP, 5 s de timeout) */
export interface UrlHealth {
  ok: boolean;
  /** Code HTTP final (0 si la requête n'a pas abouti) */
  status: number;
  error: string;
}

/** Commande rapide d'un projet : un bouton qui lance `command` dans un terminal Cockpit */
export interface ProjectCommand {
  id: number;
  project: string;
  label: string;
  command: string;
  position: number;
}

export interface ScanResult {
  path: string;
  name: string;
  compose_files: string[];
  has_dockerfile: boolean;
}

export interface CpuMetrics {
  usage_percent: number;
  cores: number;
  model_name: string;
  per_core: number[];
}

export interface MemoryMetrics {
  total: number;
  used: number;
  available: number;
  percent: number;
  swap_total: number;
  swap_used: number;
  cached: number;
  buffers: number;
  shmem: number;
  s_reclaimable: number;
  zfs_arc: number;
}

export interface DiskMetrics {
  mount: string;
  device: string;
  total: number;
  used: number;
  free: number;
  percent: number;
}

export interface ProcessMetrics {
  pid: number;
  name: string;
  cpu: number;
  memory: number;
  memory_rss: number;
  user: string;
  command: string;
  count?: number;
  children?: ProcessMetrics[];
}

export interface SystemMetrics {
  cpu: CpuMetrics;
  memory: MemoryMetrics;
  disks: DiskMetrics[];
  hostname: string;
  uptime: string;
  kernel_version: string;
  top_cpu: ProcessMetrics[];
  top_memory: ProcessMetrics[];
}

// --- Agents marketplace ---

export interface MarketplaceLocation {
  id: string;
  display_name: string;
  path: string;
  source_type: "directory" | "cache";
  editable: boolean;
  plugins_count: number;
}

export interface PluginInfo {
  marketplace: string;
  name: string;
  version: string;
  description: string;
  agents_count: number;
  editable: boolean;
}

export interface AgentInfo {
  marketplace: string;
  plugin: string;
  name: string;
  description: string;
  model: string | null;
}

export interface OrchestratorConfig {
  experimental_teams_enabled: boolean;
  teammate_mode: string;
  default_teammate_model: string | null;
  marketplaces: MarketplaceLocation[];
  enabled_plugins: string[];
}

export type RecordingState = "recording" | "transcribing" | "summarizing" | "done" | "error";

export interface Recording {
  id: number;
  project: string;
  started_at: string;
  duration_secs: number;
  state: string;
  error: string | null;
  dir: string;
}

export interface RecordingStatus {
  recording_id: number;
  project: string;
  state: RecordingState;
  error: string | null;
  started_at: string;
  /** Piste perdue au demarrage : "mic" ou "system". Un code, traduit a l'affichage. */
  lost_track?: "mic" | "system" | null;
}

export interface TerminalInfo {
  id: number;
  project: string;
  name: string;
  alive: boolean;
  /** Un CLI d'agent LLM (claude, codex, gemini...) tourne dans la session */
  llm: boolean;
}

/** Ce qu'un geste de recherche dans un terminal a trouve (commande `terminal_search`). */
export interface TerminalSearchResult {
  /** Nombre d'occurrences du motif. */
  total: number;
  /** Indice de l'occurrence courante, de la plus ancienne a la plus recente. */
  index: number | null;
  /** Ligne de la grille : 0 = premiere ligne visible, negatif = historique. */
  ligne: number | null;
  colonne: number | null;
}

export interface DefLocation {
  rel_path: string;
  /** 0-indexee (convention LSP) */
  line: number;
  character: number;
}

export interface GotoDefinitionResult {
  source: "lsp" | "search";
  hits: DefLocation[];
}

export interface DirEntry {
  name: string;
  rel_path: string;
  is_dir: boolean;
}

export interface FileContent {
  content: string;
  size: number;
  truncated: boolean;
  binary: boolean;
  /** Date de modification en millisecondes depuis epoch (0 si indisponible) */
  mtime: number;
}

/** Etat disque d'un fichier, sans son contenu : suivi du fichier affiche */
export interface FileStat {
  size: number;
  mtime: number;
}

export interface SearchNameHit {
  rel_path: string;
  is_dir: boolean;
}

export interface SearchContentHit {
  rel_path: string;
  /** 0-indexee (meme convention que DefLocation) */
  line: number;
  preview: string;
}

export interface SearchResults {
  names: SearchNameHit[];
  contents: SearchContentHit[];
  truncated: boolean;
}

export interface GitStatusEntry {
  path: string;
  status: string;
  untracked: boolean;
  staged: boolean;
  unstaged: boolean;
  additions: number;
  deletions: number;
}

export interface GitStatus {
  branch: string;
  is_repo: boolean;
  files: GitStatusEntry[];
  ahead: number | null;
  behind: number | null;
  has_upstream: boolean;
  total_additions: number;
  total_deletions: number;
}

export interface BranchInfo {
  name: string;
  current: boolean;
}

export interface CommitInfo {
  hash: string;
  full_hash: string;
  author: string;
  /** Epoch (secondes) du commit */
  epoch: number;
  /** Décorations ("HEAD -> main, tag: v1.0"), vide sinon */
  refs: string;
  subject: string;
}

export interface DiffLine {
  kind: "add" | "del" | "context";
  old_line: number | null;
  new_line: number | null;
  text: string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  hunks: DiffHunk[];
  additions: number;
  deletions: number;
}

export interface ClaudeSession {
  id: string;
  label: string;
  updated_at: number;
  renamed: boolean;
}

export interface HistoryEntry {
  command: string;
  last_used: number | null;
}

export interface DockerContainer {
  id: string;
  name: string;
  image: string;
  state: string;
  status: string;
  ports: string;
  project: string;
}

export interface DiskUsage {
  kind: string;
  total: string;
  active: string;
  size: string;
  reclaimable: string;
}

export interface DockerVolume {
  name: string;
  driver: string;
  dangling: boolean;
}

export interface DockerImage {
  id: string;
  repository: string;
  tag: string;
  size: string;
  dangling: boolean;
}

/** Fiche technique de la machine, jointe aux erreurs remontees. */
export interface MachineReport {
  app_version: string;
  distro: string;
  /** Serveur audio reellement actif : "pipewire", "pulseaudio" ou "aucun". */
  audio_server: string;
  pw_record: string;
  /** "appimage" ou "binaire". */
  packaging: string;
}
