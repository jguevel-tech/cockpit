import { writable } from "svelte/store";
import { listen } from "@tauri-apps/api/event";
import { getActiveRecording } from "../api/recorder";
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
});
