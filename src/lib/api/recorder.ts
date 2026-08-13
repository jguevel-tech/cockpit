import { invoke } from "@tauri-apps/api/core";
import type { Recording, RecordingStatus } from "../types";

// Enregistrement de reunions
export const startRecording = (project: string) => invoke<RecordingStatus>("start_recording", { project });
export const stopRecording = () => invoke("stop_recording");
export const getActiveRecording = () => invoke<RecordingStatus | null>("get_active_recording");
export const getFailedRecordings = (project: string) => invoke<Recording[]>("get_failed_recordings", { project });
export const retryRecording = (id: number) => invoke("retry_recording", { id });
export const deleteRecording = (id: number) => invoke("delete_recording", { id });

// Reglages app (cle API, prompt/modele de resume)
export const getAppSettings = () => invoke<Record<string, string>>("get_app_settings");
export const setAppSetting = (key: string, value: string) => invoke("set_app_setting", { key, value });
export const getProjectSummaryPrompt = (project: string) => invoke<string | null>("get_project_summary_prompt", { project });
export const setProjectSummaryPrompt = (project: string, prompt: string | null) =>
  invoke("set_project_summary_prompt", { project, prompt });
