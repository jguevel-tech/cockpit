//! Enregistrement de reunions : capture micro + son systeme, transcription Whisper,
//! resume LLM, depot dans une note du projet.
//!
//! Pipeline d'etats : recording -> transcribing -> summarizing -> done | error.
//! L'audio est supprime apres succes, conserve en cas d'echec (retry possible).

pub mod capture;
pub mod summarize;
pub mod transcribe;
pub mod wav;

use crate::storage::Database;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

pub struct RecorderState {
    active: Mutex<Option<ActiveRecording>>,
}

impl Default for RecorderState {
    fn default() -> Self {
        Self { active: Mutex::new(None) }
    }
}

struct ActiveRecording {
    recording_id: i64,
    project: String,
    started_at: String,
    dir: PathBuf,
    handles: capture::CaptureHandles,
}

#[derive(Serialize, Clone)]
pub struct RecordingStatus {
    pub recording_id: i64,
    pub project: String,
    pub state: String,
    pub error: Option<String>,
    pub started_at: String,
}

fn emit_status(app: &AppHandle, status: &RecordingStatus) {
    let _ = app.emit("recording_status", status.clone());
}

fn recordings_root(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app data dir: {}", e))?;
    Ok(dir.join("recordings"))
}

/// Duree d'une piste PCM brute d'apres sa taille de fichier.
fn track_duration_secs(path: &PathBuf) -> i64 {
    std::fs::metadata(path)
        .map(|m| (m.len() as usize / wav::BYTES_PER_SEC) as i64)
        .unwrap_or(0)
}

// --- Demarrage / arret ---

pub async fn start(
    app: AppHandle,
    db: Database,
    state: &RecorderState,
    project: String,
) -> Result<RecordingStatus, String> {
    {
        let active = state.active.lock().unwrap();
        if let Some(a) = active.as_ref() {
            return Err(format!("Un enregistrement est deja en cours sur \"{}\"", a.project));
        }
    }

    let started_at = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let rec = db.create_recording(&project, &started_at)?;
    let dir = recordings_root(&app)?.join(format!("rec_{}", rec.id));
    db.set_recording_dir(rec.id, &dir.to_string_lossy())?;

    let mut handles = match capture::start_capture(&dir) {
        Ok(h) => h,
        Err(e) => {
            let _ = db.delete_recording(rec.id);
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
    };

    // Laisse pw-record s'attacher aux devices ; s'il meurt tout de suite,
    // PipeWire est probablement indisponible.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    if !handles.is_alive() {
        // Ce que pw-record a dit AVANT de nettoyer : le dossier part juste apres.
        let why = handles.startup_error();
        let _ = handles.stop().await;
        let _ = db.delete_recording(rec.id);
        let _ = std::fs::remove_dir_all(&dir);
        return Err(why);
    }

    let status = RecordingStatus {
        recording_id: rec.id,
        project: project.clone(),
        state: "recording".into(),
        error: None,
        started_at: started_at.clone(),
    };

    {
        let mut active = state.active.lock().unwrap();
        *active = Some(ActiveRecording {
            recording_id: rec.id,
            project,
            started_at,
            dir,
            handles,
        });
    }

    emit_status(&app, &status);
    Ok(status)
}

pub async fn stop(app: AppHandle, db: Database, state: &RecorderState) -> Result<(), String> {
    let active = {
        let mut guard = state.active.lock().unwrap();
        guard.take().ok_or("Aucun enregistrement en cours")?
    };

    let ActiveRecording { recording_id, project, started_at, dir, handles } = active;
    handles.stop().await?;

    let duration = track_duration_secs(&dir.join("mic.raw"))
        .max(track_duration_secs(&dir.join("system.raw")));
    db.set_recording_duration(recording_id, duration)?;
    db.set_recording_state(recording_id, "transcribing", None)?;

    emit_status(
        &app,
        &RecordingStatus {
            recording_id,
            project,
            state: "transcribing".into(),
            error: None,
            started_at,
        },
    );

    tauri::async_runtime::spawn(run_pipeline(app, db, recording_id));
    Ok(())
}

pub fn active_status(state: &RecorderState) -> Option<RecordingStatus> {
    let guard = state.active.lock().unwrap();
    guard.as_ref().map(|a| RecordingStatus {
        recording_id: a.recording_id,
        project: a.project.clone(),
        state: "recording".into(),
        error: None,
        started_at: a.started_at.clone(),
    })
}

pub fn retry(app: AppHandle, db: Database, recording_id: i64) -> Result<(), String> {
    let rec = db.get_recording(recording_id)?;
    if rec.state != "error" {
        return Err("Cet enregistrement n'est pas en echec".into());
    }
    let dir = PathBuf::from(&rec.dir);
    if !dir.join("mic.raw").exists() && !dir.join("system.raw").exists() {
        return Err("Fichiers audio introuvables, retranscription impossible".into());
    }
    db.set_recording_state(recording_id, "transcribing", None)?;
    emit_status(
        &app,
        &RecordingStatus {
            recording_id,
            project: rec.project,
            state: "transcribing".into(),
            error: None,
            started_at: rec.started_at,
        },
    );
    tauri::async_runtime::spawn(run_pipeline(app, db, recording_id));
    Ok(())
}

pub fn delete(db: &Database, recording_id: i64) -> Result<(), String> {
    let rec = db.get_recording(recording_id)?;
    if !rec.dir.is_empty() {
        let _ = std::fs::remove_dir_all(&rec.dir);
    }
    db.delete_recording(recording_id)
}

// --- Pipeline transcription + resume ---

async fn run_pipeline(app: AppHandle, db: Database, recording_id: i64) {
    let rec = match db.get_recording(recording_id) {
        Ok(r) => r,
        Err(e) => {
            log::error!("recording {} introuvable: {}", recording_id, e);
            return;
        }
    };

    let result = pipeline_inner(&app, &db, &rec).await;

    let (state, error) = match &result {
        Ok(()) => ("done", None),
        Err(e) => {
            log::error!("pipeline recording {}: {}", recording_id, e);
            ("error", Some(e.as_str()))
        }
    };
    let _ = db.set_recording_state(recording_id, state, error);

    if result.is_ok() {
        let _ = std::fs::remove_dir_all(&rec.dir);
    }

    emit_status(
        &app,
        &RecordingStatus {
            recording_id,
            project: rec.project.clone(),
            state: state.into(),
            error: error.map(String::from),
            started_at: rec.started_at.clone(),
        },
    );
}

async fn pipeline_inner(
    app: &AppHandle,
    db: &Database,
    rec: &crate::storage::Recording,
) -> Result<(), String> {
    let api_key = db
        .get_setting("openai_api_key")
        .filter(|k| !k.trim().is_empty())
        .ok_or("Cle API OpenAI non configuree (Parametres globaux > Reunions)")?;

    let system_prompt = db
        .get_project_summary_prompt(&rec.project)
        .ok()
        .flatten()
        .or_else(|| db.get_setting("summary_prompt").filter(|p| !p.trim().is_empty()))
        .unwrap_or_else(|| summarize::DEFAULT_PROMPT.to_string());

    let model = db
        .get_setting("summary_model")
        .filter(|m| !m.trim().is_empty())
        .unwrap_or_else(|| summarize::DEFAULT_MODEL.to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;

    let dir = PathBuf::from(&rec.dir);
    let mic_path = dir.join("mic.raw");
    let sys_path = dir.join("system.raw");
    let (mic_res, sys_res) = futures::join!(
        transcribe::transcribe_track(&client, &api_key, &mic_path),
        transcribe::transcribe_track(&client, &api_key, &sys_path),
    );
    let (mic, sys) = (mic_res?, sys_res?);

    if mic.is_empty() && sys.is_empty() {
        return Err("Aucune parole detectee dans l'enregistrement".into());
    }

    let transcript = transcribe::merge_dialogue(mic, sys);

    emit_status(
        app,
        &RecordingStatus {
            recording_id: rec.id,
            project: rec.project.clone(),
            state: "summarizing".into(),
            error: None,
            started_at: rec.started_at.clone(),
        },
    );

    let summary = summarize::summarize(&client, &api_key, &model, &system_prompt, &transcript).await?;

    let title = note_title(&rec.started_at);
    let content = format!(
        "# {}\n\n*Durée : {}*\n\n## Résumé\n\n{}\n\n## Transcription\n\n{}",
        title,
        format_duration(rec.duration_secs),
        summary.trim(),
        transcript.trim()
    );

    // Dossier "Réunions" a la racine des notes du projet (cree au premier usage)
    let tree = db.get_note_tree(&rec.project)?;
    let folder_id = match tree
        .folders
        .iter()
        .find(|f| f.parent_id.is_none() && f.name == "Réunions")
    {
        Some(f) => f.id,
        None => db.create_note_folder(&rec.project, None, "Réunions")?.id,
    };

    let file = db.create_note_file(&rec.project, Some(folder_id), &title)?;
    db.save_note_file(file.id, &content)?;

    Ok(())
}

fn note_title(started_at: &str) -> String {
    match chrono::NaiveDateTime::parse_from_str(started_at, "%Y-%m-%d %H:%M:%S") {
        Ok(dt) => format!("Réunion du {} à {}", dt.format("%d/%m/%Y"), dt.format("%Hh%M")),
        Err(_) => format!("Réunion du {}", started_at),
    }
}

fn format_duration(secs: i64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 {
        format!("{} h {:02}", h, m)
    } else if m > 0 {
        format!("{} min", m)
    } else {
        format!("{} s", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_title() {
        assert_eq!(note_title("2026-07-08 14:30:12"), "Réunion du 08/07/2026 à 14h30");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(42), "42 s");
        assert_eq!(format_duration(300), "5 min");
        assert_eq!(format_duration(3900), "1 h 05");
    }
}
