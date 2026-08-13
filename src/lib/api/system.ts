import { invoke } from "@tauri-apps/api/core";
import type { SystemMetrics } from "../types";

export const getSystemMetrics = () => invoke<SystemMetrics>("get_system_metrics");
export const killProcess = (pid: number) => invoke("kill_process", { pid });
export const openTerminal = (path: string) => invoke("open_terminal", { path });
/// Zoom natif du webview (met a l'echelle tout le rendu, terminaux xterm compris).
export const setWebviewZoom = (factor: number) => invoke("set_webview_zoom", { factor });
