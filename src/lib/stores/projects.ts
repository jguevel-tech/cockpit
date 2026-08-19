import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { listProjects } from "../api/docker";
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

// Listen for status updates from backend
listen("status_update", async () => {
  await loadProjects();
});
