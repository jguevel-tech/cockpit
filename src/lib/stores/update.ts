import { writable } from "svelte/store";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { notify } from "./toast";
import { pushNotice, removeNoticesByPrefix } from "./notifications";
import { translate } from "../i18n";

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

/// Promesse gardee : la premiere verification peut partir avant que getVersion() ait repondu,
/// et le titre de la notice a besoin de la version installee. On l'attend explicitement plutot
/// que de lire un store qui vaudrait encore "".
const versionReady = getVersion()
  .then((v) => {
    updateState.update((s) => ({ ...s, currentVersion: v }));
    return v;
  })
  .catch((e) => {
    console.error("getVersion", e);
    return "";
  });

/// Interroge la Release GitHub la plus recente. Silencieux par defaut : au demarrage on ne
/// veut pas d'un toast d'erreur parce que la machine est hors ligne.
export async function checkForUpdate(opts: { silent?: boolean } = {}) {
  updateState.update((s) => ({ ...s, phase: "checking", error: null }));
  try {
    const current = await versionReady;
    const update = await check();
    pending = update;
    if (update) {
      updateState.update((s) => ({
        ...s,
        phase: "available",
        newVersion: update.version,
        notes: update.body ?? null,
      }));
      // L'id porte la version : une nouvelle version cree une nouvelle notice, donc non lue,
      // meme si l'utilisateur avait lu (ou ecarte) celle de la version precedente.
      pushNotice({
        id: `update:${update.version}`,
        kind: "update",
        title: current
          ? `Mise à jour disponible — ${current} → ${update.version}`
          : `Mise à jour disponible — ${update.version}`,
        body: update.body ?? undefined,
        createdAt: update.date ?? new Date().toISOString(),
        dismissible: true,
        action: { label: translate("update.install"), run: installUpdate },
      });
    } else {
      updateState.update((s) => ({ ...s, phase: "idle", newVersion: null, notes: null }));
      // Plus rien a annoncer : on retire d'eventuelles notices d'une version deja installee.
      removeNoticesByPrefix("update:");
      if (!opts.silent) notify(translate("update.upToDate"), "success");
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
    notify(translate("update.nonePending"));
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

/// Cadence de verification : demarrage, puis toutes les heures, PLUS un controle au retour de
/// focus sur la fenetre si la derniere verification a plus de 15 min.
///
/// Pourquoi pas toutes les 10 min : une release ne sort pas plus de quelques fois par jour, donc
/// marteler l'endpoint 144 fois par jour ne gagne que quelques minutes de latence. C'est le
/// controle au focus qui donne la sensation d'immediatete — on revient sur la fenetre, ca verifie.
/// Cadence comparable a VS Code (~1 h) ; Chrome est a ~5 h.
const CHECK_INTERVAL_MS = 60 * 60 * 1000;
const FOCUS_STALE_MS = 15 * 60 * 1000;

let lastCheck = 0;

async function silentCheck() {
  lastCheck = Date.now();
  await checkForUpdate({ silent: true });
}

export function startUpdateWatcher() {
  silentCheck();
  const timer = setInterval(silentCheck, CHECK_INTERVAL_MS);

  const onFocus = () => {
    if (Date.now() - lastCheck > FOCUS_STALE_MS) silentCheck();
  };
  window.addEventListener("focus", onFocus);

  return () => {
    clearInterval(timer);
    window.removeEventListener("focus", onFocus);
  };
}
