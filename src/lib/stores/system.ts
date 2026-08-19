import { writable } from "svelte/store";
import { getSystemMetrics } from "../api/system";
import type { SystemMetrics } from "../types";
import { signalerErreur } from "./errors";

const MAX_HISTORY = 60; // 60 points = 3 minutes at 3s interval
const LIVE_INTERVAL_MS = 3000;

export const systemMetrics = writable<SystemMetrics | null>(null);
export const cpuHistory = writable<number[]>([]);
export const memoryHistory = writable<number[]>([]);
export const metricsLive = writable<boolean>(false);

let liveTimer: ReturnType<typeof setInterval> | null = null;

export async function refreshMetrics() {
  try {
    const data = await getSystemMetrics();
    systemMetrics.set(data);

    cpuHistory.update((h) => {
      const next = [...h, data.cpu.usage_percent];
      return next.length > MAX_HISTORY ? next.slice(next.length - MAX_HISTORY) : next;
    });
    memoryHistory.update((h) => {
      const next = [...h, data.memory.percent];
      return next.length > MAX_HISTORY ? next.slice(next.length - MAX_HISTORY) : next;
    });
  } catch (e) {
      signalerErreur("system.refreshMetrics", String(e));
    console.error("Failed to load system metrics:", e);
  }
}

export function startLiveMetrics() {
  if (liveTimer) return;
  metricsLive.set(true);
  refreshMetrics();
  liveTimer = setInterval(refreshMetrics, LIVE_INTERVAL_MS);
}

export function stopLiveMetrics() {
  if (liveTimer) {
    clearInterval(liveTimer);
    liveTimer = null;
  }
  metricsLive.set(false);
}
