import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { getActiveRecording } from "../api/recorder";
import { notify } from "./toast";
import { translate } from "../i18n";
import type { RecordingStatus } from "../types";

// Pipeline en cours (recording/transcribing/summarizing), null sinon
export const recordingStatus = writable<RecordingStatus | null>(null);

// Dernier evenement recu, y compris les etats terminaux (done/error).
// Sert aux composants qui doivent reagir a la fin d'un pipeline (reload notes, toast).
export const lastRecordingEvent = writable<RecordingStatus | null>(null);

getActiveRecording()
  .then((s) => { if (s) recordingStatus.set(s); })
  .catch(() => {});

listen<RecordingStatus>("recording_status", (e) => {
  const s = e.payload;
  lastRecordingEvent.set(s);
  recordingStatus.set(s.state === "done" || s.state === "error" ? null : s);
  // Une piste manquante n'empeche plus d'enregistrer, mais le dire est obligatoire :
  // sans cela l'utilisateur croit capter son micro alors qu'il ne capte que le systeme.
  if (s.lost_track) {
    notify(translate(s.lost_track === "mic" ? "rec.lostMic" : "rec.lostSystem"), "info");
  }
  // Une piste qui a tourne sans recevoir un seul echantillon non nul : ce n'est pas une
  // reunion calme, c'est une capture qui n'a rien capte. Le dire des l'arret, sinon
  // l'utilisateur l'apprend par un « aucune parole detectee » qui ne montre pas la cause.
  if (s.mute_track) {
    const cle =
      s.mute_track === "both" ? "rec.muteBoth" : s.mute_track === "mic" ? "rec.muteMic" : "rec.muteSystem";
    notify(translate(cle), "error");
  }
});
