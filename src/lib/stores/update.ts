import { writable } from "svelte/store";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { notify } from "./toast";

/// Etat du telechargement, pour la barre de progression du modal.
export type UpdatePhase = "idle" | "checking" | "available" | "downloading" | "installing" | "error";

export interface UpdateState {
  phase: UpdatePhase;
  /// Version installee (vient de package.json via tauri.conf.json).
  currentVersion: string;
  /// Version proposee, null s'il n'y a rien de neuf.
  newVersion: string | null;
  /// Notes de version = section du CHANGELOG.md publiee dans la Release GitHub.
  notes: string | null;
  /// 0..100 pendant le telechargement, null si la taille totale est inconnue.
  progress: number | null;
  error: string | null;
}

const INITIAL: UpdateState = {
  phase: "idle",
  currentVersion: "",
  newVersion: null,
  notes: null,
  progress: null,
  error: null,
};

export const updateState = writable<UpdateState>(INITIAL);

/// Handle renvoye par check(), garde de cote entre la detection et l'installation :
/// downloadAndInstall() doit etre appele sur CET objet, pas sur un nouveau check().
let pending: Update | null = null;

getVersion()
  .then((v) => updateState.update((s) => ({ ...s, currentVersion: v })))
  .catch((e) => console.error("getVersion", e));

/// Interroge la Release GitHub la plus recente. Silencieux par defaut : au demarrage on ne
/// veut pas d'un toast d'erreur parce que la machine est hors ligne.
export async function checkForUpdate(opts: { silent?: boolean } = {}) {
  updateState.update((s) => ({ ...s, phase: "checking", error: null }));
  try {
    const update = await check();
    pending = update;
    if (update) {
      updateState.update((s) => ({
        ...s,
        phase: "available",
        newVersion: update.version,
        notes: update.body ?? null,
      }));
    } else {
      updateState.update((s) => ({ ...s, phase: "idle", newVersion: null, notes: null }));
      if (!opts.silent) notify("Cockpit est à jour", "success");
    }
  } catch (e) {
    updateState.update((s) => ({ ...s, phase: "idle", error: String(e) }));
    if (!opts.silent) notify(String(e));
  }
}

/// Telecharge, installe, puis relance. Sous Linux l'installation ne fonctionne que si
/// l'app tourne depuis un AppImage : un binaire brut (cargo/tauri build) n'est pas
/// remplacable, l'erreur est alors remontee telle quelle.
export async function installUpdate() {
  if (!pending) {
    notify("Aucune mise à jour en attente — relance la vérification.");
    return;
  }
  let downloaded = 0;
  let total: number | null = null;
  updateState.update((s) => ({ ...s, phase: "downloading", progress: null, error: null }));
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === "Started") {
        total = event.data.contentLength ?? null;
      } else if (event.event === "Progress") {
        downloaded += event.data.chunkLength;
        const progress = total ? Math.round((downloaded / total) * 100) : null;
        updateState.update((s) => ({ ...s, progress }));
      } else if (event.event === "Finished") {
        updateState.update((s) => ({ ...s, phase: "installing", progress: 100 }));
      }
    });
    await relaunch();
  } catch (e) {
    updateState.update((s) => ({ ...s, phase: "error", error: String(e) }));
    notify(String(e));
  }
}

/// Verification au demarrage puis toutes les 6 h. Silencieuse : la cloche apparait d'elle-meme
/// s'il y a du neuf, et une machine hors ligne ne doit pas polluer l'UI.
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

export function startUpdateWatcher() {
  checkForUpdate({ silent: true });
  const timer = setInterval(() => checkForUpdate({ silent: true }), CHECK_INTERVAL_MS);
  return () => clearInterval(timer);
}
