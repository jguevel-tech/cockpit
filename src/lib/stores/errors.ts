import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { getAppSettings, setAppSetting } from "../api/recorder";
import type { MachineReport } from "../types";

/**
 * Remontee des erreurs.
 *
 * Le point d'entree unique est `signalerErreur`, appele automatiquement par `notify(...,
 * "error")` : toute erreur affichee a l'utilisateur est donc journalisee, sans avoir a
 * penser a l'instrumenter. Cote Rust, le journal local est ecrit dans tous les cas ;
 * l'envoi au serveur de suivi n'a lieu qu'avec l'accord explicite de l'utilisateur.
 *
 * "unset" = la question n'a pas encore ete posee, ce qui declenche l'ecran d'accord.
 */
export type Consent = "on" | "off" | "unset";

const CONSENT_KEY = "error_reporting";
const USER_KEY = "error_reporting_user";

export const reportingConsent = writable<Consent>("unset");
export const reportingUser = writable<string>("");

/** Fiche machine telle qu'elle accompagne les erreurs — affichable a l'utilisateur. */
export async function machineReport(): Promise<MachineReport> {
  return invoke<MachineReport>("machine_report");
}

export async function loadReportingSettings() {
  try {
    const settings = await getAppSettings();
    const brut = settings[CONSENT_KEY];
    reportingConsent.set(brut === "on" ? "on" : brut === "off" ? "off" : "unset");
    reportingUser.set(settings[USER_KEY] ?? "");
  } catch (e) {
    // Reglages illisibles : on reste sur "unset" plutot que de supposer un accord.
    console.warn("remontee d'erreurs : reglages illisibles,", String(e));
  }
}

export async function setReportingConsent(on: boolean) {
  reportingConsent.set(on ? "on" : "off");
  await setAppSetting(CONSENT_KEY, on ? "on" : "off");
}

export async function setReportingUser(nom: string) {
  reportingUser.set(nom);
  await setAppSetting(USER_KEY, nom);
}

/**
 * Journalise une erreur, et l'envoie si l'utilisateur l'a acceptee.
 *
 * Ne rejette jamais : une remontee qui echoue ne doit pas ajouter une erreur a l'erreur.
 * `scope` situe la panne ("git.commit", "recorder.start") — c'est lui qui rend les
 * remontees exploitables plutot qu'un tas de messages sans origine.
 */
export async function signalerErreur(scope: string, message: string) {
  try {
    await invoke("report_error", { scope, message });
  } catch (e) {
    console.warn("remontee d'erreurs impossible :", String(e));
  }
}

/** Le consentement a-t-il deja ete demande ? */
export function consentDemande(): boolean {
  return get(reportingConsent) !== "unset";
}
