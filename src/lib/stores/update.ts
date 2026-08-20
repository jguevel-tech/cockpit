import { writable } from "svelte/store";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getVersion } from "@tauri-apps/api/app";
import { notify } from "./toast";
import { pushNotice, removeNoticesByPrefix } from "./notifications";
import { translate, type Catalog } from "../i18n";
import { signalerErreur } from "./errors";

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

/**
 * Cle de message lisible pour une panne de l'updater, `null` si la panne n'en a pas de
 * particuliere (l'appelant met alors son message par defaut).
 *
 * Le plugin renvoie des textes techniques anglais qui n'ont rien a faire sous les yeux d'un
 * utilisateur. Deux cas valent un message a eux :
 * - `platforms` sans notre entree : une release existe mais l'artefact de notre systeme n'y
 *   est pas encore (les jobs de plateformes ne finissent pas en meme temps). Ce n'est pas une
 *   panne, c'est « repasse dans cinq minutes ».
 * - reseau injoignable : c'est la connexion, pas le logiciel.
 *
 * Une CLE et non un texte : le message reste alors reactif au changement de langue la ou il
 * est affiche. Le detail technique n'est pas perdu, il part dans le journal.
 */
export function cleErreurMaj(brut: string): keyof Catalog | null {
  if (/platforms` object/.test(brut)) return "update.notReady";
  if (/error sending request|dns error|timed out|timeout|connection|connect |unreachable|network/i.test(brut)) {
    return "update.offline";
  }
  return null;
}

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
          ? translate("update.available", { from: current, to: update.version })
          : translate("update.availableShort", { version: update.version }),
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
    const brut = String(e);
    updateState.update((s) => ({ ...s, phase: "idle", error: brut }));
    // Journalise le texte brut (c'est lui qui sert au diagnostic) mais n'affiche que le
    // message lisible — et rien du tout si la verification etait automatique.
    void signalerErreur("update.check", brut);
    if (!opts.silent) {
      notify(translate(cleErreurMaj(brut) ?? "update.checkFailed"), "error", 6000, {
        report: false,
      });
    }
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
    const brut = String(e);
    updateState.update((s) => ({ ...s, phase: "error", error: brut }));
    void signalerErreur("update.install", brut);
    notify(translate(cleErreurMaj(brut) ?? "update.installFailed"), "error", 6000, {
      report: false,
    });
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
