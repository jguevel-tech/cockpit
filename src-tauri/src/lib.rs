mod agents;
mod appearance;
mod chemins;
mod claude_auth;
mod commande;
mod docker;
mod gitdiff;
mod lsp;
mod plugin;
mod report;
mod recorder;
mod scanner;
pub mod storage;
mod system;
mod terminal;
mod urlhealth;
mod workspace;

use commande::SansConsole;
use docker::orchestrator::Orchestrator;
use std::sync::Arc;
use storage::Database;
use system::metrics::Collector;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

// --- App State ---

pub struct AppState {
    pub db: Database,
    pub db_path: String,
    pub orchestrator: Arc<Orchestrator>,
    pub collector: Arc<Mutex<Collector>>,
    pub recorder: recorder::RecorderState,
    /// Serveur de terminaux, VU PAR LE TRAIT `Terminaux` : les commandes ci-dessous ne
    /// connaissent pas tmux. L'implementation se choisit dans `terminal::terminaux()`.
    pub terminals: Box<dyn terminal::Terminaux>,
    pub claude_login: claude_auth::ClaudeLoginState,
    pub lsp: Arc<lsp::LspState>,
}

/// Bornes du zoom webview. Doivent rester alignees sur ZOOM_LEVELS (src/lib/stores/ui.ts).
const ZOOM_MIN: f64 = 0.7;
const ZOOM_MAX: f64 = 2.0;

// --- Tauri Commands: Docker ---

#[derive(serde::Serialize)]
struct ProjectWithFolder {
    #[serde(flatten)]
    project: docker::orchestrator::Project,
    folder_id: Option<i64>,
}

#[tauri::command]
async fn list_projects(state: tauri::State<'_, AppState>) -> Result<Vec<ProjectWithFolder>, String> {
    // L'ordre vient de la DB (position), les statuts de l'orchestrateur
    let db_projects = state.db.get_projects().map_err(|e| e.to_string())?;
    let orch_projects = state.orchestrator.get_projects().await;
    let orch_map: std::collections::HashMap<String, docker::orchestrator::Project> =
        orch_projects.into_iter().map(|p| (p.name.clone(), p)).collect();

    // Un projet EN BASE doit TOUJOURS apparaitre, meme si l'orchestrateur ne le connait pas
    // (son add_project peut echouer silencieusement a la creation). L'ancienne version ne
    // gardait que l'intersection : le projet fraichement cree devenait invisible du frontend,
    // donc onglet Docker vide et bouton + du terminal inerte — constate chez le premier
    // utilisateur externe le 2026-08-14. En secours on synthetise une entree arretee avec le
    // chemin de la base, ce qui suffit aux terminaux, fichiers et git.
    let mut result: Vec<ProjectWithFolder> = db_projects
        .iter()
        .map(|db_p| ProjectWithFolder {
            project: orch_map.get(&db_p.name).cloned().unwrap_or_else(|| {
                docker::orchestrator::Project {
                    name: db_p.name.clone(),
                    path: db_p.path.clone(),
                    description: db_p.description.clone(),
                    depends_on: db_p.depends_on.clone(),
                    depended_by: Vec::new(),
                    state: docker::orchestrator::ProjectState::Stopped,
                    containers: Vec::new(),
                    error: String::new(),
                    has_compose: docker::compose::Compose::new(&db_p.path, &db_p.compose_file)
                        .has_compose_file(),
                }
            }),
            folder_id: db_p.folder_id,
        })
        .collect();

    // Ajouter les projets orphelins (dans l'orchestrateur mais pas en DB)
    for (name, proj) in &orch_map {
        if !db_projects.iter().any(|p| &p.name == name) {
            result.push(ProjectWithFolder {
                project: proj.clone(),
                folder_id: None,
            });
        }
    }

    Ok(result)
}

#[tauri::command]
async fn start_project(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.orchestrator.start_project(&name).await
}

#[tauri::command]
async fn stop_project(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.orchestrator.stop_project(&name).await
}

#[tauri::command]
async fn restart_project(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.orchestrator.restart_project(&name).await
}

// --- Tauri Commands: Conteneurs Docker (vue globale) ---

#[tauri::command]
async fn list_all_containers() -> Result<Vec<docker::containers::DockerContainer>, String> {
    docker::containers::list_all().await
}

#[tauri::command]
async fn container_action(id: String, action: String) -> Result<(), String> {
    docker::containers::container_action(&id, &action).await
}

#[tauri::command]
async fn container_logs(id: String, tail: u32) -> Result<String, String> {
    docker::containers::container_logs(&id, tail).await
}

#[tauri::command]
async fn container_action_bulk(ids: Vec<String>, action: String) -> Result<(), String> {
    docker::containers::container_action_bulk(&ids, &action).await
}

#[tauri::command]
async fn docker_disk_usage() -> Result<Vec<docker::containers::DiskUsage>, String> {
    docker::containers::disk_usage().await
}

#[tauri::command]
async fn list_docker_volumes() -> Result<Vec<docker::containers::DockerVolume>, String> {
    docker::containers::list_volumes().await
}

#[tauri::command]
async fn list_docker_images() -> Result<Vec<docker::containers::DockerImage>, String> {
    docker::containers::list_images().await
}

#[tauri::command]
async fn remove_docker_volume(name: String) -> Result<(), String> {
    docker::containers::remove_volume(&name).await
}

#[tauri::command]
async fn remove_docker_image(id: String) -> Result<(), String> {
    docker::containers::remove_image(&id).await
}

#[tauri::command]
async fn docker_prune(target: String) -> Result<String, String> {
    docker::containers::prune(&target).await
}

// --- Tauri Commands: Todos ---

#[tauri::command]
fn get_todos(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::Todo>, String> {
    state.db.get_todos(&project)
}

#[tauri::command]
fn create_todo(project: String, text: String, state: tauri::State<'_, AppState>) -> Result<storage::Todo, String> {
    state.db.create_todo(&project, &text)
}

#[tauri::command]
fn update_todo(id: i64, text: String, done: bool, state: tauri::State<'_, AppState>) -> Result<storage::Todo, String> {
    state.db.update_todo(id, &text, done)
}

#[tauri::command]
fn set_todo_due(id: i64, due_date: Option<String>, state: tauri::State<'_, AppState>) -> Result<storage::Todo, String> {
    state.db.set_todo_due(id, due_date.as_deref())
}

#[tauri::command]
fn delete_todo(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.delete_todo(id)
}

#[tauri::command]
fn reorder_todos(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_todos(&ids)
}

#[tauri::command]
fn move_todo(id: i64, new_project: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.move_todo(id, &new_project)
}

#[tauri::command]
fn get_pending_todos(state: tauri::State<'_, AppState>) -> Result<Vec<storage::Todo>, String> {
    state.db.get_pending_todos()
}

// --- Tauri Commands: Notes ---

#[tauri::command]
fn get_note(project: String, state: tauri::State<'_, AppState>) -> Result<Option<storage::Note>, String> {
    state.db.get_note(&project)
}

#[tauri::command]
fn save_note(project: String, content: String, state: tauri::State<'_, AppState>) -> Result<storage::Note, String> {
    state.db.save_note(&project, &content)
}

#[tauri::command]
fn get_note_tree(project: String, state: tauri::State<'_, AppState>) -> Result<storage::NoteTree, String> {
    state.db.get_note_tree(&project)
}

#[tauri::command]
fn create_note_folder(project: String, parent_id: Option<i64>, name: String, state: tauri::State<'_, AppState>) -> Result<storage::NoteFolder, String> {
    state.db.create_note_folder(&project, parent_id, &name)
}

#[tauri::command]
fn rename_note_folder(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.rename_note_folder(id, &name)
}

#[tauri::command]
fn delete_note_folder(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.delete_note_folder(id)
}

#[tauri::command]
fn create_note_file(project: String, folder_id: Option<i64>, name: String, state: tauri::State<'_, AppState>) -> Result<storage::NoteFile, String> {
    state.db.create_note_file(&project, folder_id, &name)
}

#[tauri::command]
fn get_note_file(id: i64, state: tauri::State<'_, AppState>) -> Result<storage::NoteFile, String> {
    state.db.get_note_file(id)
}

#[tauri::command]
fn save_note_file(id: i64, content: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.save_note_file(id, &content)
}

#[tauri::command]
fn rename_note_file(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.rename_note_file(id, &name)
}

#[tauri::command]
fn delete_note_file(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.delete_note_file(id)
}

#[tauri::command]
fn reorder_note_folders(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_note_folders(&ids)
}

#[tauri::command]
fn reorder_note_files(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_note_files(&ids)
}

#[tauri::command]
fn move_note_file(id: i64, folder_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.move_note_file(id, folder_id)
}

// --- Tauri Commands: URLs ---

#[tauri::command]
fn get_urls(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::Url>, String> {
    state.db.get_urls(&project)
}

#[tauri::command]
async fn check_urls(urls: Vec<String>) -> Vec<urlhealth::UrlHealth> {
    urlhealth::check_urls(&urls).await
}

#[tauri::command]
fn create_url(project: String, label: String, url: String, state: tauri::State<'_, AppState>) -> Result<storage::Url, String> {
    state.db.create_url(&project, &label, &url)
}

#[tauri::command]
fn update_url(id: i64, label: String, url: String, state: tauri::State<'_, AppState>) -> Result<storage::Url, String> {
    state.db.update_url(id, &label, &url)
}

#[tauri::command]
fn delete_url(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.delete_url(id)
}

// --- Tauri Commands: Commandes rapides par projet ---

#[tauri::command]
fn get_project_commands(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::ProjectCommand>, String> {
    state.db.get_project_commands(&project)
}

#[tauri::command]
fn create_project_command(project: String, label: String, command: String, state: tauri::State<'_, AppState>) -> Result<storage::ProjectCommand, String> {
    state.db.create_project_command(&project, &label, &command)
}

#[tauri::command]
fn update_project_command(id: i64, label: String, command: String, state: tauri::State<'_, AppState>) -> Result<storage::ProjectCommand, String> {
    state.db.update_project_command(id, &label, &command)
}

#[tauri::command]
fn delete_project_command(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.delete_project_command(id)
}

#[tauri::command]
fn reorder_project_commands(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_project_commands(&ids)
}

// --- Tauri Commands: Project Folders ---

#[tauri::command]
fn get_project_folders(state: tauri::State<'_, AppState>) -> Result<Vec<storage::ProjectFolder>, String> {
    state.db.get_project_folders()
}

#[tauri::command]
fn create_project_folder(name: String, parent_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<storage::ProjectFolder, String> {
    state.db.create_project_folder(&name, parent_id)
}

#[tauri::command]
fn rename_project_folder(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.rename_project_folder(id, &name)
}

#[tauri::command]
fn delete_project_folder(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.delete_project_folder(id)
}

#[tauri::command]
fn reorder_project_folders(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_project_folders(&ids)
}

/// Deplace un dossier sous un autre (`parent_id` a None = racine). Refuse les boucles.
#[tauri::command]
fn move_project_folder(id: i64, parent_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.move_project_folder(id, parent_id)
}

#[tauri::command]
fn move_project_to_folder(project_name: String, folder_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.move_project_to_folder(&project_name, folder_id)
}

// --- Tauri Commands: Scanner ---

#[tauri::command]
fn scan_dir(path: String) -> Result<scanner::ScanResult, String> {
    scanner::scan(&path)
}

#[tauri::command]
fn scan_subdirs(path: String) -> Result<Vec<scanner::ScanResult>, String> {
    scanner::scan_subdirs(&path)
}

// --- Tauri Commands: Settings (DB projects) ---

#[tauri::command]
fn get_db_projects(state: tauri::State<'_, AppState>) -> Result<Vec<storage::Project>, String> {
    state.db.get_projects()
}

#[tauri::command]
async fn add_project(
    name: String,
    path: String,
    compose_file: String,
    description: String,
    depends_on: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<storage::Project, String> {
    let proj = state.db.create_project(&name, &path, &compose_file, &description, &depends_on)?;
    // Seul echec possible : le nom existe deja dans l'orchestrateur (creation precedente
    // avortee, suppression partielle). Mettre a jour au lieu d'avaler l'erreur — c'est ce
    // silence qui rendait un projet frais invisible/fige chez le premier utilisateur externe.
    if state
        .orchestrator
        .add_project(&name, &path, &compose_file, &description, depends_on.clone())
        .await
        .is_err()
    {
        state.orchestrator.update_project(&name, &path, &compose_file, &description, depends_on).await;
    }
    Ok(proj)
}

#[tauri::command]
fn update_db_project(
    id: i64,
    name: String,
    path: String,
    compose_file: String,
    description: String,
    depends_on: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<storage::Project, String> {
    state.db.update_project(id, &name, &path, &compose_file, &description, &depends_on)
}

#[tauri::command]
async fn delete_db_project(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let name = state.db.get_project_by_id(id).ok().map(|p| p.name);

    // Tue les sessions tmux vivantes du projet (leurs lignes DB partent avec le projet)
    if let Some(name) = &name {
        for t in state.terminals.lister(&state.db, Some(name)) {
            let _ = state.terminals.fermer(&state.db, t.id);
        }
    }

    // Supprime le projet + toutes ses donnees (cascade par nom en DB)
    state.db.delete_project(id)?;

    // Retire de l'orchestrateur (sinon subsiste comme projet orphelin en sidebar)
    if let Some(name) = name {
        let _ = state.orchestrator.remove_project(&name).await;
    }
    Ok(())
}

#[tauri::command]
fn reorder_projects(names: Vec<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.reorder_projects(&names)
}

/// Nom DB reel d'un projet a partir de son nom AFFICHE (orchestrateur, en
/// memoire). Le nom affiche peut avoir derive du nom stocke (renommages) :
/// on retombe alors sur le chemin, identite stable — meme logique que
/// rename_project. Sans correspondance, retourne le nom affiche tel quel.
async fn resolve_db_project_name(state: &tauri::State<'_, AppState>, display_name: &str) -> String {
    if state.db.get_project_by_name(display_name).is_ok() {
        return display_name.to_string();
    }
    let path = state
        .orchestrator
        .get_projects()
        .await
        .into_iter()
        .find(|p| p.name == display_name)
        .map(|p| p.path)
        .filter(|p| !p.is_empty());
    if let Some(p) = path {
        if let Ok(db_name) = state.db.get_project_name_by_path(&p) {
            return db_name;
        }
    }
    display_name.to_string()
}

#[tauri::command]
async fn get_project_settings(name: String, state: tauri::State<'_, AppState>) -> Result<storage::Project, String> {
    let db_name = resolve_db_project_name(&state, &name).await;
    state.db.get_project_by_name(&db_name)
}

#[tauri::command]
async fn update_project_settings(
    name: String,
    path: String,
    compose_file: String,
    description: String,
    depends_on: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<storage::Project, String> {
    let db_name = resolve_db_project_name(&state, &name).await;
    let proj = state.db.update_project_by_name(&db_name, &path, &compose_file, &description, &depends_on)?;
    // L'orchestrateur est indexe par le nom AFFICHE, lui
    state.orchestrator.update_project(&name, &path, &compose_file, &description, depends_on).await;
    Ok(proj)
}

#[tauri::command]
async fn rename_project(
    old_name: String,
    new_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Chemin du projet (identite stable) pour retrouver la ligne DB meme si le
    // nom affiche a derive du nom stocke.
    let path = state
        .orchestrator
        .get_projects()
        .await
        .into_iter()
        .find(|p| p.name == old_name)
        .map(|p| p.path);
    state.db.rename_project(&old_name, &new_name, path.as_deref())?;
    state.orchestrator.rename_project(&old_name, &new_name).await;
    Ok(())
}

// --- Tauri Commands: System ---

#[tauri::command]
async fn get_system_metrics(state: tauri::State<'_, AppState>) -> Result<system::metrics::SystemMetrics, String> {
    let mut collector = state.collector.lock().await;
    Ok(collector.collect())
}

#[tauri::command]
async fn kill_process(pid: u32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let collector = state.collector.lock().await;
    system::process::kill_process_with_sys(collector.system(), pid)
}

// --- Tauri Commands: Apparence (image de fond) ---

/// Resout `<app_data>`, ou toutes les donnees de l'app vivent deja (DB, enregistrements, tmux.conf).
fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir indisponible : {}", e))
}

#[tauri::command]
fn set_wallpaper(app: tauri::AppHandle, data_url: String) -> Result<(), String> {
    appearance::set_wallpaper(&app_data_dir(&app)?, &data_url)
}

#[tauri::command]
fn get_wallpaper(app: tauri::AppHandle) -> Result<Option<String>, String> {
    appearance::get_wallpaper(&app_data_dir(&app)?)
}

#[tauri::command]
fn clear_wallpaper(app: tauri::AppHandle) -> Result<(), String> {
    appearance::clear_wallpaper(&app_data_dir(&app)?)
}

#[tauri::command]
fn read_image_as_data_url(path: String) -> Result<String, String> {
    appearance::read_image_as_data_url(&path)
}

// --- Tauri Commands: Zoom ---

/// Zoom natif du webview (equivalent Ctrl+molette d'un navigateur) : met a l'echelle
/// TOUT le rendu — typo, paddings, bordures et terminaux xterm compris.
/// Choisi plutot qu'un `html { font-size }` variable parce que ~423 tailles px
/// (paddings, --header-height, boutons 32x32) ne suivraient pas les rem et le texte
/// finirait par deborder de ses boites.
/// Cote terminaux : rien a faire, changer le zoom change les dimensions en px CSS du
/// conteneur -> le ResizeObserver de TerminalTab refit et renvoie la taille a tmux.
#[tauri::command]
fn set_webview_zoom(window: tauri::WebviewWindow, factor: f64) -> Result<(), String> {
    if !(ZOOM_MIN..=ZOOM_MAX).contains(&factor) {
        return Err(format!("facteur de zoom hors bornes : {}", factor));
    }
    window.set_zoom(factor).map_err(|e| e.to_string())
}

// --- Tauri Command: Terminal ---

#[tauri::command]
async fn open_terminal(path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);
    if !path.is_dir() {
        return Err("Le repertoire n'existe pas".to_string());
    }
    let path_str = path.to_string_lossy().to_string();

    // Try gnome-terminal with two tabs
    let result = tokio::process::Command::new("gnome-terminal")
        .sans_console()
        .arg("--tab")
        .arg("--title=Terminal")
        .arg(format!("--working-directory={}", &path_str))
        .arg("--tab")
        .arg("--title=Claude Code")
        .arg(format!("--working-directory={}", &path_str))
        .arg("--")
        .arg("bash")
        .arg("-c")
        .arg("claude; exec bash")
        .spawn();

    if result.is_ok() {
        return Ok(());
    }

    // Fallback: x-terminal-emulator (single tab)
    let result = tokio::process::Command::new("x-terminal-emulator")
        .sans_console()
        .arg("-e")
        .arg("bash")
        .current_dir(&path_str)
        .spawn();

    if result.is_ok() {
        return Ok(());
    }

    Err("Aucun terminal trouve (gnome-terminal, x-terminal-emulator)".to_string())
}

// --- Tauri Command: Import DB ---

#[tauri::command]
fn import_database(path: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    storage::import::import_from(&state.db, &path)
}

#[tauri::command]
fn get_db_path(state: tauri::State<'_, AppState>) -> String {
    state.db_path.clone()
}

// --- Tauri Commands: Enregistrement de reunions ---

#[tauri::command]
async fn start_recording(
    app: tauri::AppHandle,
    project: String,
    state: tauri::State<'_, AppState>,
) -> Result<recorder::RecordingStatus, String> {
    recorder::start(app, state.db.clone(), &state.recorder, project).await
}

#[tauri::command]
async fn stop_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    recorder::stop(app, state.db.clone(), &state.recorder).await
}

#[tauri::command]
fn get_active_recording(state: tauri::State<'_, AppState>) -> Option<recorder::RecordingStatus> {
    recorder::active_status(&state.recorder)
}

#[tauri::command]
fn get_failed_recordings(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::Recording>, String> {
    state.db.get_failed_recordings(&project)
}

#[tauri::command]
fn retry_recording(app: tauri::AppHandle, id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    recorder::retry(app, state.db.clone(), id)
}

#[tauri::command]
fn delete_recording(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    recorder::delete(&state.db, id)
}

// --- Tauri Commands: App settings (cle API, prompt de resume) ---

#[tauri::command]
fn get_app_settings(state: tauri::State<'_, AppState>) -> Result<std::collections::HashMap<String, String>, String> {
    let mut settings = state.db.get_all_settings()?;
    settings
        .entry("summary_prompt".into())
        .or_insert_with(|| recorder::summarize::DEFAULT_PROMPT.to_string());
    settings
        .entry("summary_model".into())
        .or_insert_with(|| recorder::summarize::DEFAULT_MODEL.to_string());
    Ok(settings)
}

#[tauri::command]
fn set_app_setting(key: String, value: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.set_setting(&key, &value)
}

#[tauri::command]
async fn get_project_summary_prompt(project: String, state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    let db_name = resolve_db_project_name(&state, &project).await;
    state.db.get_project_summary_prompt(&db_name)
}

#[tauri::command]
async fn set_project_summary_prompt(project: String, prompt: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db_name = resolve_db_project_name(&state, &project).await;
    state.db.set_project_summary_prompt(&db_name, prompt.as_deref())
}

// --- Tauri Commands: Terminaux integres ---
//
// `async fn` N'EST PAS DECORATIF ICI. Une commande declaree `fn` (sans async) est executee
// EN LIGNE par le gestionnaire IPC de wry, qui est un signal GTK : elle tourne donc sur la
// boucle principale et gele toute l'interface — y compris la livraison de la sortie des
// terminaux — le temps de son execution. Mesure du 2026-08-20 : `list_all_terminals`, appelee
// toutes les 5 s par le magasin `terminals`, prenait 50 ms a vide et jusqu'a 1 s sous la
// charge d'agents en cours. Regle : toute commande qui LANCE UN PROCESS EXTERNE (tmux, git,
// docker...) ou qui fait des entrees-sorties bornees par autre chose que la memoire est
// `async fn`. Restent `fn` celles qui ne touchent que la base ou un champ en memoire, ainsi
// que `write_terminal` : c'est le chemin de frappe, il ne fait aucun fork et un aller-retour
// vers le runtime asynchrone lui ajouterait de la latence pour rien.

#[tauri::command]
async fn create_terminal(
    project: String,
    cwd: String,
    cols: u16,
    rows: u16,
    init_command: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<i64, String> {
    let demande = terminal::Creation {
        projet: project,
        dossier: cwd,
        taille: terminal::Taille { colonnes: cols, lignes: rows },
        commande_initiale: init_command,
    };
    state.terminals.creer(&state.db, demande)
}

#[tauri::command]
fn write_terminal(id: i64, data: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.terminals.ecrire(id, &data)
}

#[tauri::command]
async fn resize_terminal(id: i64, cols: u16, rows: u16, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .terminals
        .redimensionner(&state.db, id, terminal::Taille { colonnes: cols, lignes: rows })
}

#[tauri::command]
async fn close_terminal(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.terminals.fermer(&state.db, id)
}

#[tauri::command]
async fn attach_terminal(
    id: i64,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .terminals
        .attacher(&state.db, id, terminal::Taille { colonnes: cols, lignes: rows })
}

#[tauri::command]
fn rename_terminal(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.terminals.renommer(&state.db, id, &name)
}

#[tauri::command]
async fn list_terminals(
    project: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<terminal::TerminalInfo>, String> {
    Ok(state.terminals.lister(&state.db, Some(&project)))
}

#[tauri::command]
async fn list_all_terminals(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<terminal::TerminalInfo>, String> {
    Ok(state.terminals.lister(&state.db, None))
}

/// Presse-papier systeme. Instance arboard gardee en vie : sous X11 le contenu
/// du presse-papier disparait quand son proprietaire (la connexion) est droppe.
static CLIPBOARD: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);

#[tauri::command]
fn set_clipboard(text: String) -> Result<(), String> {
    poser_presse_papier(text)
}

/// Le meme geste, appelable depuis le backend : un programme qui demande la copie par
/// OSC 52 passe par le service de terminaux, pas par une commande IPC.
pub fn poser_presse_papier(text: String) -> Result<(), String> {
    let mut guard = CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    guard
        .as_mut()
        .unwrap()
        .set_text(text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_clipboard() -> Result<String, String> {
    let mut guard = CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    // Presse-papier vide = chaine vide (pas une erreur)
    Ok(guard.as_mut().unwrap().get_text().unwrap_or_default())
}

#[tauri::command]
fn list_claude_sessions(
    project_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<workspace::claude_sessions::ClaudeSession>, String> {
    workspace::claude_sessions::list_claude_sessions(&state.db, &project_path)
}

#[tauri::command]
fn rename_claude_session(
    session_id: String,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    workspace::claude_sessions::rename_claude_session(&state.db, &session_id, &name)
}

#[tauri::command]
fn record_command(project: String, command: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    terminal::history::record(&state.db, &project, &command)
}

#[tauri::command]
async fn terminal_search(
    id: i64,
    action: String,
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<terminal::ResultatRecherche, String> {
    let action = terminal::ActionRecherche::depuis_texte(&action)?;
    state.terminals.chercher(&state.db, id, action, &query)
}

// --- Tauri Commands: Connexion Claude Code (abonnement) ---

#[tauri::command]
fn claude_auth_status() -> claude_auth::ClaudeAuthStatus {
    claude_auth::status()
}

#[tauri::command]
fn start_claude_login(app: tauri::AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.claude_login.start(app)
}

#[tauri::command]
fn claude_login_input(data: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.claude_login.input(&data)
}

#[tauri::command]
fn cancel_claude_login(state: tauri::State<'_, AppState>) {
    state.claude_login.cancel()
}

/// Schemas d'adresse que l'application accepte de confier au systeme.
///
/// `http` est la pour les liens de dev locaux (localhost:8060...). `mailto` est la parce
/// qu'une note en fabrique tout seule : marked autolinke les adresses mail, donc un compte
/// rendu de reunion qui cite un correspondant contient un `mailto:` que le frontend presente
/// comme ouvrable. Le refuser ici affichait une erreur technique sur un lien parfaitement
/// legitime. xdg-open sait le passer au client mail par defaut.
///
/// Le schema seul ne suffit pas : `mailto:` sans destinataire ouvrirait un brouillon vide.
fn schema_ouvrable(url: &str) -> bool {
    ["https://", "http://", "mailto:"]
        .iter()
        .any(|schema| url.len() > schema.len() && url.starts_with(schema))
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !schema_ouvrable(&url) {
        return Err(format!("adresse non ouvrable : {url}"));
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
/// Journalise une erreur et la remonte au serveur de suivi si l'utilisateur l'a accepte.
///
/// Le JOURNAL est ecrit dans tous les cas : il marche hors ligne, ne demande aucun accord,
/// et c'est lui qu'on relit quand une erreur est signalee de vive voix. L'ENVOI, lui, est
/// conditionne a l'accord explicite, et refuse un transport en clair (voir report::send).
///
/// N'echoue jamais : une remontee cassee ne doit pas ajouter une erreur a l'erreur.
async fn report_error(
    app: tauri::AppHandle,
    scope: String,
    message: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    // Tout ce qui touche la base est lu AVANT le point d'attente : la connexion n'est pas
    // faite pour traverser un await.
    let (autorise, utilisateur) = {
        let db = &state.db;
        let autorise = db.get_setting(report::CONSENT_KEY).as_deref() == Some("on");
        let utilisateur = db
            .get_setting(report::USER_KEY)
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| whoami_fallback());
        (autorise, utilisateur)
    };

    if let Ok(dir) = app_data_dir(&app) {
        let horodatage = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        report::append_log(&dir, &report::format_log_line(&horodatage, &scope, &message));
    }

    if autorise {
        report::send(&scope, &message, &utilisateur).await;
    }
    Ok(())
}

/// Nom par defaut a cote des erreurs : le compte du systeme, faute de mieux.
/// `USERNAME` est la variable de Windows, `USER`/`LOGNAME` celles d'Unix.
fn whoami_fallback() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "inconnu".to_string())
}

#[tauri::command]
/// Fiche technique de la machine, telle qu'elle accompagne les erreurs.
/// Sert aussi a l'afficher a l'utilisateur : il doit pouvoir voir ce qui serait envoye.
fn machine_report() -> report::MachineInfo {
    report::machine_info().clone()
}

#[tauri::command]
fn debug_log(line: String) {
    use std::io::Write;
    // `temp_dir()` et pas `/tmp` : sous Windows le dossier temporaire est dans le profil de
    // l'utilisateur, et un chemin absolu `/tmp` y designerait la racine du lecteur courant.
    let chemin = std::env::temp_dir().join("cockpit-debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(chemin) {
        let _ = writeln!(f, "{}", line);
    }
}

#[tauri::command]
fn search_command_history(
    query: String,
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Vec<terminal::history::HistoryEntry> {
    terminal::history::search(&state.db, &query, limit.unwrap_or(50))
}

// --- Tauri Commands: Explorateur de fichiers ---

#[tauri::command]
fn list_project_dir(project_path: String, rel_path: String) -> Result<Vec<workspace::DirEntry>, String> {
    workspace::list_dir(&project_path, &rel_path)
}

#[tauri::command]
fn read_project_file(project_path: String, rel_path: String) -> Result<workspace::FileContent, String> {
    workspace::read_project_file(&project_path, &rel_path)
}

/// Etat disque du fichier affiche : sert au suivi des modifications exterieures
/// (relire 2 Mo toutes les deux secondes serait absurde, un stat ne coute rien).
#[tauri::command]
fn stat_project_file(
    project_path: String,
    rel_path: String,
) -> Result<Option<workspace::FileStat>, String> {
    workspace::stat_project_file(&project_path, &rel_path)
}

#[tauri::command]
fn write_project_file(project_path: String, rel_path: String, content: String) -> Result<(), String> {
    workspace::write_project_file(&project_path, &rel_path, &content)
}

#[tauri::command]
fn read_project_image(project_path: String, rel_path: String) -> Result<String, String> {
    workspace::read_project_image(&project_path, &rel_path)
}

#[tauri::command]
fn backup_database(dest: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.db.backup_to(&dest)
}

#[tauri::command]
fn create_project_file(project_path: String, rel_dir: String, name: String) -> Result<String, String> {
    workspace::create_project_file(&project_path, &rel_dir, &name)
}

#[tauri::command]
fn create_project_dir(project_path: String, rel_dir: String, name: String) -> Result<String, String> {
    workspace::create_project_dir(&project_path, &rel_dir, &name)
}

#[tauri::command]
fn rename_project_entry(project_path: String, rel_path: String, new_name: String) -> Result<String, String> {
    workspace::rename_project_entry(&project_path, &rel_path, &new_name)
}

#[tauri::command]
fn trash_project_entry(project_path: String, rel_path: String) -> Result<(), String> {
    workspace::trash_project_entry(&project_path, &rel_path)
}

// async : la recherche parcourt tout le projet, elle ne doit pas bloquer le thread principal
#[tauri::command]
async fn search_project(project_path: String, query: String) -> Result<workspace::SearchResults, String> {
    tokio::task::spawn_blocking(move || workspace::search_project(&project_path, &query))
        .await
        .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize, Clone)]
struct GotoDefinitionResult {
    /// "lsp" ou "search" (repli heuristique)
    source: String,
    hits: Vec<lsp::DefLocation>,
}

/// Aller a la definition : LSP si un serveur existe pour le langage, sinon
/// recherche heuristique de declarations. `content` = texte courant du viewer
/// (positions coherentes meme avec des modifications non sauvees).
#[tauri::command]
async fn goto_definition(
    project_path: String,
    lang: String,
    rel_path: String,
    content: String,
    line: u32,
    character: u32,
    symbol: String,
    state: tauri::State<'_, AppState>,
) -> Result<GotoDefinitionResult, String> {
    let lsp_state = state.lsp.clone();
    tokio::task::spawn_blocking(move || {
        if lsp::available(&lang) {
            match lsp_state.definition(&project_path, &lang, &rel_path, &content, line, character) {
                Ok(hits) if !hits.is_empty() => {
                    return Ok(GotoDefinitionResult { source: "lsp".into(), hits });
                }
                Ok(_) => {} // pas de resultat LSP -> repli
                Err(e) if e.contains("delai") => return Err(e), // indexation : ne pas polluer avec le repli
                Err(_) => {} // serveur en erreur -> repli
            }
        }
        let hits = workspace::find_symbol(&project_path, &symbol)?
            .into_iter()
            .map(|h| lsp::DefLocation { rel_path: h.rel_path, line: h.line, character: 0 })
            .collect();
        Ok(GotoDefinitionResult { source: "search".into(), hits })
    })
    .await
    .map_err(|e| e.to_string())?
}

// --- Tauri Commands: Git ---

#[tauri::command]
async fn git_status(project_path: String) -> Result<gitdiff::GitStatus, String> {
    gitdiff::git_status(&project_path).await
}

#[tauri::command]
async fn git_diff_file(
    project_path: String,
    path: String,
    untracked: bool,
) -> Result<gitdiff::FileDiff, String> {
    gitdiff::git_diff_file(&project_path, &path, untracked).await
}

#[tauri::command]
async fn git_stage(project_path: String, path: String) -> Result<(), String> {
    gitdiff::git_stage(&project_path, &path).await
}

#[tauri::command]
async fn git_unstage(project_path: String, path: String) -> Result<(), String> {
    gitdiff::git_unstage(&project_path, &path).await
}

#[tauri::command]
async fn git_stage_all(project_path: String) -> Result<(), String> {
    gitdiff::git_stage_all(&project_path).await
}

#[tauri::command]
async fn git_unstage_all(project_path: String) -> Result<(), String> {
    gitdiff::git_unstage_all(&project_path).await
}

#[tauri::command]
async fn git_commit(project_path: String, message: String) -> Result<(), String> {
    gitdiff::git_commit(&project_path, &message).await
}

#[tauri::command]
async fn git_push(project_path: String, set_upstream: bool) -> Result<String, String> {
    gitdiff::git_push(&project_path, set_upstream).await
}

#[tauri::command]
async fn git_pull(project_path: String) -> Result<String, String> {
    gitdiff::git_pull(&project_path).await
}

#[tauri::command]
async fn git_log(project_path: String, limit: u32) -> Result<Vec<gitdiff::CommitInfo>, String> {
    gitdiff::git_log(&project_path, limit).await
}

#[tauri::command]
async fn git_commit_diff(project_path: String, hash: String) -> Result<Vec<gitdiff::FileDiff>, String> {
    gitdiff::git_commit_diff(&project_path, &hash).await
}

#[tauri::command]
async fn git_branches(project_path: String) -> Result<Vec<gitdiff::BranchInfo>, String> {
    gitdiff::git_branches(&project_path).await
}

#[tauri::command]
async fn git_checkout_branch(project_path: String, name: String) -> Result<(), String> {
    gitdiff::git_checkout_branch(&project_path, &name).await
}

#[tauri::command]
async fn git_create_branch(project_path: String, name: String) -> Result<(), String> {
    gitdiff::git_create_branch(&project_path, &name).await
}

#[tauri::command]
async fn git_delete_branch(project_path: String, name: String, force: bool) -> Result<(), String> {
    gitdiff::git_delete_branch(&project_path, &name, force).await
}

// --- Tauri Commands: Agents marketplace (multi-marketplace) ---

#[tauri::command]
fn get_marketplace_path() -> Result<String, String> {
    Ok(agents::ccm_marketplace_path()?.to_string_lossy().to_string())
}

#[tauri::command]
fn list_marketplaces() -> Result<Vec<agents::MarketplaceLocation>, String> {
    agents::list_marketplaces()
}

#[tauri::command]
fn list_plugins(marketplace_id: String) -> Result<Vec<agents::PluginInfo>, String> {
    agents::list_plugins_in(&marketplace_id)
}

#[tauri::command]
fn list_agents(marketplace_id: String, plugin: String) -> Result<Vec<agents::AgentInfo>, String> {
    agents::list_agents_in(&marketplace_id, &plugin)
}

#[tauri::command]
fn read_agent(marketplace_id: String, plugin: String, name: String) -> Result<String, String> {
    agents::read_agent(&marketplace_id, &plugin, &name)
}

#[tauri::command]
fn save_agent(
    marketplace_id: String,
    plugin: String,
    name: String,
    content: String,
) -> Result<(), String> {
    agents::save_agent(&marketplace_id, &plugin, &name, &content)
}

#[tauri::command]
fn delete_agent(marketplace_id: String, plugin: String, name: String) -> Result<(), String> {
    agents::delete_agent(&marketplace_id, &plugin, &name)
}

#[tauri::command]
fn rename_agent(
    marketplace_id: String,
    plugin: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    agents::rename_agent(&marketplace_id, &plugin, &old_name, &new_name)
}

#[tauri::command]
fn create_plugin(name: String, description: String) -> Result<(), String> {
    agents::create_plugin(&name, &description)
}

#[tauri::command]
fn delete_plugin(marketplace_id: String, name: String) -> Result<(), String> {
    agents::delete_plugin(&marketplace_id, &name)
}

#[tauri::command]
fn rename_plugin(
    marketplace_id: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    agents::rename_plugin(&marketplace_id, &old_name, &new_name)
}

#[tauri::command]
fn get_project_plugins(project_path: String) -> Result<Vec<String>, String> {
    agents::get_project_plugins(&project_path)
}

#[tauri::command]
fn set_project_plugins(project_path: String, plugins: Vec<String>) -> Result<(), String> {
    agents::set_project_plugins(&project_path, plugins)
}

#[tauri::command]
fn get_orchestrator_config() -> Result<agents::OrchestratorConfig, String> {
    agents::get_orchestrator_config()
}

#[tauri::command]
fn set_teams_enabled(enabled: bool) -> Result<(), String> {
    agents::set_teams_enabled(enabled)
}

#[tauri::command]
fn set_teammate_mode(mode: String) -> Result<(), String> {
    agents::set_teammate_mode(&mode)
}

#[tauri::command]
fn toggle_plugin_enabled(plugin_key: String, enabled: bool) -> Result<(), String> {
    agents::toggle_plugin_enabled(&plugin_key, enabled)
}

// --- App Setup ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Ecarte les polices emoji en couleur du format COLRv1, pour l'interface de Cockpit
/// UNIQUEMENT.
///
/// Le moteur de rendu embarque dans l'AppImage est plus ancien que ces polices : sur Fedora
/// (`Noto-COLRv1.ttf`), il echoue sur une assertion interne au moment de dessiner un emoji
/// dans le terminal, et la fenetre gele — sans trace, puisque l'assertion tue le processus de
/// rendu. Confirme par un utilisateur : en ecartant cette police, l'onglet Terminal
/// redevient utilisable.
///
/// On n'agit QUE dans l'AppImage (c'est son moteur qui est en cause, pas celui du systeme)
/// et QUE pour notre processus : la configuration du systeme n'est pas touchee, les autres
/// programmes gardent leurs emoji en couleur. Les notres s'affichent alors avec la police de
/// repli — moins joli qu'un gel de l'application.
#[cfg(target_os = "linux")]
fn ecarter_polices_colrv1() {
    if std::env::var_os("APPDIR").is_none() {
        return;
    }
    // Respecte une configuration deja choisie par l'utilisateur.
    if std::env::var_os("FONTCONFIG_FILE").is_some() {
        return;
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
        });
    let Some(dir) = base.map(|b| b.join("com.cockpit.dev")) else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let chemin = dir.join("fonts.conf");
    if std::fs::write(&chemin, configuration_polices()).is_err() {
        return;
    }
    std::env::set_var("FONTCONFIG_FILE", &chemin);
}

/// Contenu de la configuration de polices : celle du systeme, moins les polices COLRv1.
/// Linux seulement — fontconfig n'existe ni sous Windows ni sous macOS, et son seul
/// appelant (`ecarter_polices_colrv1`) porte deja ce `cfg`.
#[cfg(target_os = "linux")]
fn configuration_polices() -> String {
    // `include` d'abord : sans la configuration du systeme, plus AUCUNE police n'est
    // trouvee et l'interface s'affiche en carres.
    concat!(
        "<?xml version=\"1.0\"?>\n",
        "<!DOCTYPE fontconfig SYSTEM \"fonts.dtd\">\n",
        "<fontconfig>\n",
        "  <include ignore_missing=\"yes\">/etc/fonts/fonts.conf</include>\n",
        "  <selectfont><rejectfont><glob>*COLRv1*</glob></rejectfont></selectfont>\n",
        "</fontconfig>\n",
    )
    .to_string()
}

/// Precharge la libwayland-client DU SYSTEME pour les process enfants.
///
/// Le WebKitWebProcess, qui fait le rendu, est un process enfant : il herite de
/// LD_PRELOAD et se lie ainsi a la meme version que le pilote graphique de l'hote,
/// au lieu de celle qu'embarque l'AppImage. Sans cela il abort au demarrage sur les
/// distributions a Mesa 25+ (voir l'appelant).
///
/// Ne fait rien hors AppImage (les bibliotheques sont alors deja celles du systeme),
/// ni si l'hote ne fournit pas la bibliotheque : on preserve le comportement actuel
/// plutot que de risquer un prechargement impossible.
#[cfg(target_os = "linux")]
fn preload_system_libwayland() {
    if std::env::var_os("APPDIR").is_none() {
        return;
    }

    const CANDIDATES: [&str; 3] = [
        "/usr/lib/x86_64-linux-gnu/libwayland-client.so.0",
        "/usr/lib64/libwayland-client.so.0",
        "/usr/lib/libwayland-client.so.0",
    ];

    let Some(lib) = CANDIDATES
        .iter()
        .find(|path| std::path::Path::new(path).exists())
    else {
        return;
    };

    let value = match std::env::var("LD_PRELOAD") {
        Ok(existing) if !existing.is_empty() => format!("{lib}:{existing}"),
        _ => (*lib).to_string(),
    };
    std::env::set_var("LD_PRELOAD", value);
}

/// Le meme binaire sert de SERVICE DE TERMINAUX quand on le lance avec
/// `--service-terminaux <socket>` : c'est ainsi que l'application se relance elle-meme,
/// detachee, pour que les shells lui survivent (`terminal/service/lancement.rs`).
///
/// A appeler en TOUT PREMIER dans `main` : rien de Tauri, de GTK ni de la base ne doit
/// etre initialise dans ce processus, qui n'ouvre aucune fenetre. Rend `true` quand le
/// processus vient de faire son travail et doit s'arreter.
///
/// Sans cet argument, ne fait rien : le processus continue et ouvre l'application.
pub fn service_terminaux_si_demande() -> bool {
    terminal::service::lancement::tourner_si_demande()
}

pub fn run() {
    // FIX RACINE bug accents terminaux (NE PAS RETIRER) : sous Linux, ibus route
    // les touches accentuees DIRECTES de l'AZERTY (é è ç à) par le pipeline de
    // composition IME du WebView, en emettant des compositionend SANS
    // compositionstart — un cas que xterm.js gere mal (accumulation du textarea,
    // prefixes espace+insecable, doublons). Le contexte de saisie simple de GTK
    // (integre, gere aussi les touches mortes ^+e -> ê) supprime toute
    // composition pour ces touches : frappes normales, zero artefact.
    // Doit etre pose AVANT l'init GTK (donc avant le Builder).
    #[cfg(target_os = "linux")]
    std::env::set_var("GTK_IM_MODULE", "gtk-im-context-simple");

    // L'AppImage embarque la libwayland-client de sa machine de construction (Ubuntu
    // 22.04 -> 1.20). Sur une distro plus recente, le pilote graphique du systeme
    // (Mesa 25+, lui jamais embarque) se retrouve lie a cette vieille version :
    // eglGetPlatformDisplay rend EGL_BAD_PARAMETER, WebKit fait un abort volontaire
    // ("Could not create default EGL display") et le WebKitWebProcess meurt — l'hote
    // affiche son rapporteur de plantage et la fenetre ne s'ouvre jamais. Constate sur
    // Ubuntu 26.04 ; reproduit en conteneur : 22.04 et 24.04 demarrent, 26.04 abort a
    // chaque fois. Bug amont sans correctif ni option d'exclusion (tauri-apps/tauri#15665),
    // d'ou ce contournement ici. Doit etre pose AVANT l'init GTK.
    #[cfg(target_os = "linux")]
    preload_system_libwayland();

    // Le moteur de rendu embarque gele sur les polices emoji COLRv1 des distributions
    // recentes (Fedora). Doit etre pose AVANT l'init GTK, comme les reglages ci-dessus.
    #[cfg(target_os = "linux")]
    ecarter_polices_colrv1();

    // Un panic Rust ne passe par aucun `catch` de l'interface : sans ce filet, il ne
    // laissait aucune trace. Le journal local est ecrit ici meme (l'envoi, lui, demande un
    // runtime async qui n'existe pas forcement a cet instant : la ligne journalisee est
    // relue au signalement).
    {
        let precedent = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // Le dossier est celui que Tauri a resolu, memorise pendant le `setup` : a
            // l'instant d'un panic on ne peut pas compter sur le handle Tauri (raison qui
            // n'a pas change), mais on n'a plus besoin de RECONSTRUIRE un chemin. La
            // version precedente rejouait `XDG_DATA_HOME` -> `~/.local/share`, juste sous
            // Linux : sur macOS elle ecrivait dans un dossier que personne ne relit, sous
            // Windows nulle part.
            let dir = chemins::dossier_donnees().cloned().unwrap_or_else(|| {
                // Un panic AVANT le `setup` (init des plugins) n'a pas encore de dossier de
                // donnees memorise. Le temporaire est le seul endroit portable et toujours
                // accessible : mieux qu'une trace perdue, et la fenetre se compte en
                // millisecondes.
                std::env::temp_dir().join("cockpit")
            });
            let horodatage = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            report::append_log(
                &dir,
                &report::format_log_line(&horodatage, "panic", &info.to_string()),
            );
            precedent(info);
        }));
    }

    tauri::Builder::default()
        // UNE SEULE INSTANCE, et ce n'est pas cosmetique : deux Cockpit partagent la meme
        // base ET le meme serveur tmux, or `purge_dead` TUE au demarrage les sessions
        // `ckpt_*` absentes de sa base. Une seconde instance (a plus forte raison avec une
        // autre base) detruisait donc les terminaux de la premiere. Constate sur une machine
        // ou cinq instances tournaient en parallele, dont deux depuis cinq jours.
        // Le lancement suivant redonne le focus a la fenetre existante au lieu d'ouvrir un
        // second exemplaire.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            use tauri::Manager;
            if let Some(fenetre) = app.webview_windows().values().next() {
                let _ = fenetre.unminimize();
                let _ = fenetre.show();
                let _ = fenetre.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        // Mise a jour automatique : verifie la Release GitHub la plus recente, telecharge
        // et installe l'AppImage signe. `process` sert a relancer l'app juste apres.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // EN PREMIER : le hook de panic ecrit dans ce dossier, et le `expect` de
            // l'ouverture de la base, juste dessous, est une source de panic reelle.
            if let Ok(dir) = app.path().app_data_dir() {
                chemins::memoriser_dossier_donnees(dir);
            }

            // Check for --db CLI argument or env var, otherwise use app data dir
            let db_path = std::env::var("COCKPIT_DB")
                .ok()
                .or_else(|| {
                    std::env::args().skip_while(|a| a != "--db").nth(1)
                })
                .unwrap_or_else(|| {
                    let app_dir = app.path().app_data_dir().unwrap();
                    std::fs::create_dir_all(&app_dir).ok();
                    app_dir.join("data.db").to_string_lossy().to_string()
                });

            log::info!("Using database: {}", db_path);
            let db = Database::new(&db_path)
                .expect("failed to open database");

            // Enregistrements restes en plein pipeline a la fermeture -> erreur (retry possible)
            let _ = db.fail_stale_recordings();

            // Serveur de terminaux : mise en route (lancement du service s'il ne tourne
            // pas deja, puis reconciliation avec la base) avant toute autre operation.
            let terminaux = terminal::terminaux();
            terminaux.preparer(app.handle(), &db);

            // Import initial de la cle API depuis secrets.json (depose manuellement)
            if db.get_setting("openai_api_key").filter(|k| !k.is_empty()).is_none() {
                if let Ok(app_dir) = app.path().app_data_dir() {
                    if let Ok(raw) = std::fs::read_to_string(app_dir.join("secrets.json")) {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) {
                            if let Some(key) = json.get("openai_api_key").and_then(|v| v.as_str()) {
                                let _ = db.set_setting("openai_api_key", key);
                                log::info!("openai_api_key importee depuis secrets.json");
                            }
                        }
                    }
                }
            }

            // Load projects from DB
            let db_projects = db.get_projects().unwrap_or_default();
            let project_defs: Vec<_> = db_projects
                .iter()
                .map(|p| {
                    (
                        p.name.clone(),
                        p.path.clone(),
                        p.compose_file.clone(),
                        p.description.clone(),
                        p.depends_on.clone(),
                    )
                })
                .collect();

            // Init orchestrator
            let orchestrator = Arc::new(
                Orchestrator::new(&project_defs).expect("failed to create orchestrator"),
            );

            // Init system collector
            let collector = Arc::new(Mutex::new(Collector::new()));

            // Store state
            app.manage(AppState {
                db,
                db_path: db_path.clone(),
                orchestrator: orchestrator.clone(),
                collector,
                recorder: recorder::RecorderState::default(),
                terminals: terminaux,
                claude_login: claude_auth::ClaudeLoginState::default(),
                lsp: Arc::new(lsp::LspState::default()),
            });

            // Start status monitor (every 5s)
            let orch_clone = orchestrator.clone();
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                docker::monitor::start_status_monitor(orch_clone, 5, move || {
                    let _ = app_handle.emit("status_update", ());
                })
                .await;
            });

            // System metrics are now on-demand (no background ticker)

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Docker
            list_projects,
            start_project,
            stop_project,
            restart_project,
            list_all_containers,
            container_action,
            container_logs,
            container_action_bulk,
            docker_disk_usage,
            list_docker_volumes,
            list_docker_images,
            remove_docker_volume,
            remove_docker_image,
            docker_prune,
            // Todos
            get_todos,
            create_todo,
            update_todo,
            set_todo_due,
            delete_todo,
            reorder_todos,
            move_todo,
            get_pending_todos,
            // Notes
            get_note,
            save_note,
            get_note_tree,
            create_note_folder,
            rename_note_folder,
            delete_note_folder,
            create_note_file,
            get_note_file,
            save_note_file,
            rename_note_file,
            delete_note_file,
            reorder_note_folders,
            reorder_note_files,
            move_note_file,
            // URLs
            get_urls,
            create_url,
            check_urls,
            get_project_commands,
            create_project_command,
            update_project_command,
            delete_project_command,
            reorder_project_commands,
            update_url,
            delete_url,
            // Project Folders
            get_project_folders,
            create_project_folder,
            rename_project_folder,
            delete_project_folder,
            reorder_project_folders,
            move_project_folder,
            move_project_to_folder,
            // Scanner
            scan_dir,
            scan_subdirs,
            // Settings
            get_db_projects,
            add_project,
            update_db_project,
            delete_db_project,
            reorder_projects,
            get_project_settings,
            rename_project,
            update_project_settings,
            // System
            get_system_metrics,
            kill_process,
            // Zoom
            set_webview_zoom,
            // Apparence
            set_wallpaper,
            get_wallpaper,
            clear_wallpaper,
            read_image_as_data_url,
            // Terminal
            open_terminal,
            // Migration
            import_database,
            get_db_path,
            // Enregistrement de reunions
            start_recording,
            stop_recording,
            get_active_recording,
            get_failed_recordings,
            retry_recording,
            delete_recording,
            get_app_settings,
            set_app_setting,
            get_project_summary_prompt,
            set_project_summary_prompt,
            // Terminaux integres
            create_terminal,
            write_terminal,
            resize_terminal,
            close_terminal,
            attach_terminal,
            rename_terminal,
            list_terminals,
            list_all_terminals,
            set_clipboard,
            get_clipboard,
            terminal_search,
            list_claude_sessions,
            rename_claude_session,
            record_command,
            search_command_history,
            debug_log,
            report_error,
            machine_report,
            // Connexion Claude Code
            claude_auth_status,
            start_claude_login,
            claude_login_input,
            cancel_claude_login,
            open_url,
            // Explorateur de fichiers
            list_project_dir,
            read_project_file,
            stat_project_file,
            search_project,
            read_project_image,
            backup_database,
            create_project_file,
            create_project_dir,
            rename_project_entry,
            trash_project_entry,
            write_project_file,
            goto_definition,
            // Git
            git_status,
            git_diff_file,
            git_stage,
            git_unstage,
            git_stage_all,
            git_unstage_all,
            git_commit,
            git_push,
            git_pull,
            git_log,
            git_commit_diff,
            git_branches,
            git_checkout_branch,
            git_create_branch,
            git_delete_branch,
            // Agents marketplace (multi)
            get_marketplace_path,
            list_marketplaces,
            list_plugins,
            list_agents,
            read_agent,
            save_agent,
            delete_agent,
            rename_agent,
            create_plugin,
            delete_plugin,
            rename_plugin,
            get_project_plugins,
            set_project_plugins,
            get_orchestrator_config,
            set_teams_enabled,
            set_teammate_mode,
            toggle_plugin_enabled,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            // Fermeture : stoppe les serveurs LSP (sinon intelephense &
            // rust-analyzer survivent en orphelins)
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    state.lsp.shutdown_all();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "linux")]
    fn la_configuration_de_polices_inclut_celle_du_systeme() {
        // Sans cet include, plus aucune police n'est trouvee : l'interface s'affiche en
        // carres, ce qui serait pire que le defaut corrige.
        let conf = super::configuration_polices();
        assert!(conf.contains("/etc/fonts/fonts.conf"), "{conf}");
        assert!(conf.contains("ignore_missing"), "{conf}");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn la_configuration_ecarte_les_polices_colrv1() {
        let conf = super::configuration_polices();
        assert!(conf.contains("rejectfont"), "{conf}");
        assert!(conf.contains("*COLRv1*"), "{conf}");
    }

    #[test]
    fn une_adresse_mail_est_ouvrable() {
        // marked autolinke les adresses mail des notes : le cas arrive sans que personne
        // n'ecrive un lien a la main, et le frontend l'annonce comme ouvrable.
        assert!(super::schema_ouvrable("mailto:bob@ex.com"));
        assert!(super::schema_ouvrable("mailto:bob@ex.com?subject=Reunion"));
    }

    #[test]
    fn le_web_reste_ouvrable() {
        assert!(super::schema_ouvrable("https://example.com"));
        assert!(super::schema_ouvrable("http://localhost:8060/"));
    }

    #[test]
    fn les_adresses_incompletes_ou_dangereuses_sont_refusees() {
        for url in [
            "www.ex.com",       // lien de note sans schema
            "../doc.md",        // lien relatif
            "file:///etc/passwd",
            "javascript:alert(1)",
            "mailto:",          // brouillon vide
            "https://",
            "",
        ] {
            assert!(!super::schema_ouvrable(url), "{url}");
        }
    }
}
