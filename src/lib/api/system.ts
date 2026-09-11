import { invoke } from "../coquille";
import type { SystemMetrics } from "../types";

export const getSystemMetrics = () => invoke<SystemMetrics>("get_system_metrics");
export const killProcess = (pid: number) => invoke("kill_process", { pid });
/// Zoom natif du webview (met a l'echelle tout le rendu, terminaux xterm compris).
export const setWebviewZoom = (factor: number) => invoke("coquille:zoom", { factor });
