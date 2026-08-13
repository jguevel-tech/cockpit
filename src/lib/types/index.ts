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
}

export interface Todo {
  id: number;
  project: string;
  text: string;
  done: boolean;
  position: number;
  created_at: string;
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

export interface SitemapPair {
  id: number;
  project: string;
  label: string;
  sitemap_ref_url: string;
  sitemap_check_url: string;
  ref_query: string;
  check_query: string;
  position: number;
  limit_urls: number | null;
}

// Champs editables d'une paire (creation / edition). snake_case aligne sur le Rust.
export interface SitemapPairInput {
  label: string;
  sitemap_ref_url: string;
  sitemap_check_url: string;
  ref_query: string;
  check_query: string;
  limit_urls: number | null;
}

export interface PingItem {
  url: string;
  status_code: number | null;
  ok: boolean;
  error: string | null;
  duration_ms: number;
}

export interface PingReport {
  pair_id: number;
  total: number;
  ok: number;
  ko: number;
  items: PingItem[];
}

export type DiffStatus = "Equal" | "Different" | "OrphanRef" | "OrphanCheck" | "Error";

export interface DiffItem {
  path: string;
  ref_url: string | null;
  check_url: string | null;
  status: DiffStatus;
  ref_bytes: number | null;
  check_bytes: number | null;
  diff: string | null;
  error: string | null;
}

export interface DiffReport {
  pair_id: number;
  total: number;
  equal: number;
  different: number;
  orphans: number;
  errors: number;
  items: DiffItem[];
}

export interface SitemapProgress {
  pair_id: number;
  mode: "ping" | "diff";
  done: number;
  total: number;
  current_url: string;
  status: "ok" | "ko" | "equal" | "different" | "orphan_ref" | "orphan_check" | "error";
  detail: string;
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
}

export interface TerminalInfo {
  id: number;
  project: string;
  name: string;
  alive: boolean;
  /** Un CLI d'agent LLM (claude, codex, gemini...) tourne dans la session */
  llm: boolean;
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
