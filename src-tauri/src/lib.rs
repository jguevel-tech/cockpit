mod agents;
mod appearance;
mod evenements;
mod taches;
pub mod pont;
mod chemins;
pub mod compte;
mod commande;
mod docker;
mod fenetre;
mod gitdiff;
mod guetteur;
mod llm;
mod lsp;
mod plugin;
mod report;
mod recorder;
mod rendu;
mod scanner;
pub mod storage;
mod system;
mod terminal;
mod urlhealth;
mod workspace;

use docker::orchestrator::Orchestrator;
use std::sync::Arc;
use storage::Database;
use system::metrics::Collector;
#[cfg(feature = "interface-tauri")]
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
    /// La connexion guidee en cours, quel que soit le fournisseur.
    pub connexion_llm: llm::abonnement::SessionConnexion,
    pub lsp: Arc<lsp::LspState>,
    /// De quoi parler a l'interface, quelle qu'elle soit. **C'est ce qui remplace
    /// l'`AppHandle` que se passaient les commandes** : l'enregistrement et la connexion
    /// guidee doivent emettre, et rien d'autre ne les liait a Tauri.
    pub emetteur: crate::evenements::Emetteurs,
}

/// Sert le pont si `--pont` est demande, et dit si ce processus lui appartient.
///
/// **RIEN NE DOIT ALLER SUR LA SORTIE STANDARD EN DEHORS DU PROTOCOLE** : elle EST le
/// tuyau. Les pannes partent donc sur la sortie d'erreur, que l'hote peut journaliser
/// sans casser sa lecture.
pub fn pont_si_demande() -> bool {
    if !std::env::args().any(|a| a == "--pont") {
        return false;
    }
    match tokio::runtime::Runtime::new() {
        Ok(runtime) => {
            if let Err(e) = runtime.block_on(pont::servir()) {
                eprintln!("pont : {e}");
            }
        }
        Err(e) => eprintln!("pont : runtime impossible a demarrer : {e}"),
    }
    true
}

/// Le chemin de la base, dans l'ordre : `COCKPIT_DB`, puis `--db`, puis le dossier de
/// donnees. Extrait du `setup` pour que le pont applique la MEME regle : deux resolutions
/// du meme chemin finiraient par diverger, et l'une des deux ouvrirait une base vide sans
/// que rien ne le signale.
pub fn chemin_de_la_base(dossier_donnees: &std::path::Path) -> String {
    std::env::var("COCKPIT_DB")
        .ok()
        .or_else(|| std::env::args().skip_while(|a| a != "--db").nth(1))
        .unwrap_or_else(|| {
            std::fs::create_dir_all(dossier_donnees).ok();
            dossier_donnees.join("data.db").to_string_lossy().to_string()
        })
}

/// Construit l'etat applicatif. **Aucun hote en particulier n'est suppose ici** : ni
/// `AppHandle`, ni fenetre, ni Tauri. Ce qui reste au `setup` de l'appelant, ce sont les
/// EFFETS qui precedent (ouvrir la base, mettre le guetteur en marche, preparer les
/// terminaux), parce qu'ils dependent de l'hote et pas de l'etat.
///
/// L'orchestrateur reste construit ici et non passe en argument : il derive des projets
/// que la base contient deja, donc le calculer dehors donnerait deux facons de l'obtenir.
pub fn construire_etat(
    db: Database,
    db_path: String,
    terminaux: Box<dyn terminal::Terminaux>,
    emetteur: crate::evenements::Emetteurs,
) -> AppState {
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

    AppState {
        db,
        db_path,
        orchestrator: Arc::new(
            Orchestrator::new(&project_defs).expect("failed to create orchestrator"),
        ),
        collector: Arc::new(Mutex::new(Collector::new())),
        recorder: recorder::RecorderState::default(),
        terminals: terminaux,
        connexion_llm: llm::abonnement::SessionConnexion::default(),
        lsp: Arc::new(lsp::LspState::default()),
        emetteur,
    }
}


/// Bornes du zoom webview. Doivent rester alignees sur ZOOM_LEVELS (src/lib/stores/ui.ts).
const ZOOM_MIN: f64 = 0.7;
const ZOOM_MAX: f64 = 2.0;

// --- Tauri Commands: Docker ---

#[derive(serde::Serialize)]
pub struct ProjectWithFolder {
    #[serde(flatten)]
    project: docker::orchestrator::Project,
    folder_id: Option<i64>,
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn list_projects(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ProjectWithFolder>, String> {
    list_projects_pour_hote(&state).await
}

/// La logique de `list_projects`, appelable par TOUT hote. Le corps n'a pas bouge : seule
/// la signature change, `&AppState` se lisant comme `State<AppState>` par deref.
pub async fn list_projects_pour_hote(
    state: &AppState,
) -> Result<Vec<ProjectWithFolder>, String> {
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

/// La langue imposee au demarrage, ou `None`.
///
/// **Elle existe pour le harnais de captures, et pour rien d'autre.** Le choix de langue vit
/// dans le `localStorage`, donc DANS la WebView : impossible a poser depuis l'exterieur avant le
/// premier rendu. Sans cette variable, le harnais devrait piloter les menus pour changer de
/// langue — or il paie deja cinq pieges de ce genre, et une capture qui depend de la position
/// d'une entree de menu casse au premier remaniement de l'interface.
///
/// Rendue `fn` et non `async fn` : elle lit une variable d'environnement, elle ne touche ni la
/// base ni un process externe. Une valeur inconnue est traitee comme absente, pour qu'une faute
/// de frappe ne fasse pas demarrer l'interface dans une langue vide.
#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn langue_imposee() -> Option<String> {
    langue_imposee_reelle()
}

/// La langue imposee par l'environnement, hors de toute commande : le pont la sert aussi,
/// et deux lectures de la meme variable finiraient par diverger.
pub fn langue_imposee_reelle() -> Option<String> {
    langue_valide(&std::env::var("COCKPIT_LANGUE").ok()?)
}

/// La langue si elle est connue, `None` sinon.
///
/// Separee de la lecture d'environnement pour etre essayable : une variable d'environnement est
/// un etat GLOBAL, et les essais rust tournent en parallele — l'un poserait la variable pendant
/// qu'un autre la lit.
fn langue_valide(valeur: &str) -> Option<String> {
    match valeur.trim() {
        "fr" => Some("fr".to_string()),
        "en" => Some("en".to_string()),
        _ => None,
    }
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn start_project(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    start_project_pour_hote(&state, name).await
}

/// La logique de `start_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn start_project_pour_hote(state: &AppState, name: String) -> Result<(), String> {
    state.orchestrator.start_project(&name).await
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn stop_project(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    stop_project_pour_hote(&state, name).await
}

/// La logique de `stop_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn stop_project_pour_hote(state: &AppState, name: String) -> Result<(), String> {
    state.orchestrator.stop_project(&name).await
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn restart_project(name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    restart_project_pour_hote(&state, name).await
}

/// La logique de `restart_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn restart_project_pour_hote(state: &AppState, name: String) -> Result<(), String> {
    state.orchestrator.restart_project(&name).await
}

// --- Tauri Commands: Conteneurs Docker (vue globale) ---

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn list_all_containers() -> Result<Vec<docker::containers::DockerContainer>, String> {
    docker::containers::list_all().await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn container_action(id: String, action: String) -> Result<(), String> {
    docker::containers::container_action(&id, &action).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn container_logs(id: String, tail: u32) -> Result<String, String> {
    docker::containers::container_logs(&id, tail).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn container_action_bulk(ids: Vec<String>, action: String) -> Result<(), String> {
    docker::containers::container_action_bulk(&ids, &action).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn docker_disk_usage() -> Result<Vec<docker::containers::DiskUsage>, String> {
    docker::containers::disk_usage().await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn list_docker_volumes() -> Result<Vec<docker::containers::DockerVolume>, String> {
    docker::containers::list_volumes().await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn list_docker_images() -> Result<Vec<docker::containers::DockerImage>, String> {
    docker::containers::list_images().await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn remove_docker_volume(name: String) -> Result<(), String> {
    docker::containers::remove_volume(&name).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn remove_docker_image(id: String) -> Result<(), String> {
    docker::containers::remove_image(&id).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn docker_prune(target: String) -> Result<String, String> {
    docker::containers::prune(&target).await
}

// --- Tauri Commands: Todos ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_todos(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::Todo>, String> {
    get_todos_pour_hote(&state, project)
}

/// La logique de `get_todos`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_todos_pour_hote(state: &AppState, project: String) -> Result<Vec<storage::Todo>, String> {
    state.db.get_todos(&project)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn create_todo(project: String, text: String, state: tauri::State<'_, AppState>) -> Result<storage::Todo, String> {
    create_todo_pour_hote(&state, project, text)
}

/// La logique de `create_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn create_todo_pour_hote(state: &AppState, project: String, text: String) -> Result<storage::Todo, String> {
    state.db.create_todo(&project, &text)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn update_todo(id: i64, text: String, done: bool, state: tauri::State<'_, AppState>) -> Result<storage::Todo, String> {
    update_todo_pour_hote(&state, id, text, done)
}

/// La logique de `update_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn update_todo_pour_hote(state: &AppState, id: i64, text: String, done: bool) -> Result<storage::Todo, String> {
    state.db.update_todo(id, &text, done)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn set_todo_due(id: i64, due_date: Option<String>, state: tauri::State<'_, AppState>) -> Result<storage::Todo, String> {
    set_todo_due_pour_hote(&state, id, due_date)
}

/// La logique de `set_todo_due`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn set_todo_due_pour_hote(state: &AppState, id: i64, due_date: Option<String>) -> Result<storage::Todo, String> {
    state.db.set_todo_due(id, due_date.as_deref())
}

/// Avancement d'une tache, en pourcentage. 100 la marque finie.
#[cfg(feature = "interface-tauri")]
#[tauri::command]
fn set_todo_progress(id: i64, progress: i32, state: tauri::State<'_, AppState>) -> Result<storage::Todo, String> {
    set_todo_progress_pour_hote(&state, id, progress)
}

/// La logique de `set_todo_progress`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn set_todo_progress_pour_hote(state: &AppState, id: i64, progress: i32) -> Result<storage::Todo, String> {
    state.db.set_todo_progress(id, progress)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn delete_todo(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_todo_pour_hote(&state, id)
}

/// La logique de `delete_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn delete_todo_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_todo(id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn reorder_todos(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    reorder_todos_pour_hote(&state, ids)
}

/// La logique de `reorder_todos`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn reorder_todos_pour_hote(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_todos(&ids)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn move_todo(id: i64, new_project: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    move_todo_pour_hote(&state, id, new_project)
}

/// La logique de `move_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn move_todo_pour_hote(state: &AppState, id: i64, new_project: String) -> Result<(), String> {
    state.db.move_todo(id, &new_project)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_pending_todos(state: tauri::State<'_, AppState>) -> Result<Vec<storage::Todo>, String> {
    get_pending_todos_pour_hote(&state)
}

/// La logique de `get_pending_todos`, appelable par TOUT hote. La commande Tauri
/// ci-dessus n'en est que la facade, et le pont appelle celle-ci : une seule verite.
pub fn get_pending_todos_pour_hote(state: &AppState) -> Result<Vec<storage::Todo>, String> {
    state.db.get_pending_todos()
}

// --- Tauri Commands: Notes ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_note(project: String, state: tauri::State<'_, AppState>) -> Result<Option<storage::Note>, String> {
    get_note_pour_hote(&state, project)
}

/// La logique de `get_note`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_note_pour_hote(state: &AppState, project: String) -> Result<Option<storage::Note>, String> {
    state.db.get_note(&project)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn save_note(project: String, content: String, state: tauri::State<'_, AppState>) -> Result<storage::Note, String> {
    save_note_pour_hote(&state, project, content)
}

/// La logique de `save_note`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn save_note_pour_hote(state: &AppState, project: String, content: String) -> Result<storage::Note, String> {
    state.db.save_note(&project, &content)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_note_tree(project: String, state: tauri::State<'_, AppState>) -> Result<storage::NoteTree, String> {
    get_note_tree_pour_hote(&state, project)
}

/// La logique de `get_note_tree`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_note_tree_pour_hote(state: &AppState, project: String) -> Result<storage::NoteTree, String> {
    state.db.get_note_tree(&project)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn create_note_folder(project: String, parent_id: Option<i64>, name: String, state: tauri::State<'_, AppState>) -> Result<storage::NoteFolder, String> {
    create_note_folder_pour_hote(&state, project, parent_id, name)
}

/// La logique de `create_note_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn create_note_folder_pour_hote(state: &AppState, project: String, parent_id: Option<i64>, name: String) -> Result<storage::NoteFolder, String> {
    state.db.create_note_folder(&project, parent_id, &name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn rename_note_folder(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    rename_note_folder_pour_hote(&state, id, name)
}

/// La logique de `rename_note_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn rename_note_folder_pour_hote(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.db.rename_note_folder(id, &name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn delete_note_folder(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_note_folder_pour_hote(&state, id)
}

/// La logique de `delete_note_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn delete_note_folder_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_note_folder(id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn create_note_file(project: String, folder_id: Option<i64>, name: String, state: tauri::State<'_, AppState>) -> Result<storage::NoteFile, String> {
    create_note_file_pour_hote(&state, project, folder_id, name)
}

/// La logique de `create_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn create_note_file_pour_hote(state: &AppState, project: String, folder_id: Option<i64>, name: String) -> Result<storage::NoteFile, String> {
    state.db.create_note_file(&project, folder_id, &name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_note_file(id: i64, state: tauri::State<'_, AppState>) -> Result<storage::NoteFile, String> {
    get_note_file_pour_hote(&state, id)
}

/// La logique de `get_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_note_file_pour_hote(state: &AppState, id: i64) -> Result<storage::NoteFile, String> {
    state.db.get_note_file(id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn save_note_file(id: i64, content: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    save_note_file_pour_hote(&state, id, content)
}

/// La logique de `save_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn save_note_file_pour_hote(state: &AppState, id: i64, content: String) -> Result<(), String> {
    state.db.save_note_file(id, &content)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn rename_note_file(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    rename_note_file_pour_hote(&state, id, name)
}

/// La logique de `rename_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn rename_note_file_pour_hote(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.db.rename_note_file(id, &name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn delete_note_file(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_note_file_pour_hote(&state, id)
}

/// La logique de `delete_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn delete_note_file_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_note_file(id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn reorder_note_folders(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    reorder_note_folders_pour_hote(&state, ids)
}

/// La logique de `reorder_note_folders`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn reorder_note_folders_pour_hote(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_note_folders(&ids)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn reorder_note_files(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    reorder_note_files_pour_hote(&state, ids)
}

/// La logique de `reorder_note_files`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn reorder_note_files_pour_hote(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_note_files(&ids)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn move_note_file(id: i64, folder_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    move_note_file_pour_hote(&state, id, folder_id)
}

/// La logique de `move_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn move_note_file_pour_hote(state: &AppState, id: i64, folder_id: Option<i64>) -> Result<(), String> {
    state.db.move_note_file(id, folder_id)
}

// --- Tauri Commands: URLs ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_urls(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::Url>, String> {
    get_urls_pour_hote(&state, project)
}

/// La logique de `get_urls`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_urls_pour_hote(state: &AppState, project: String) -> Result<Vec<storage::Url>, String> {
    state.db.get_urls(&project)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn check_urls(urls: Vec<String>) -> Vec<urlhealth::UrlHealth> {
    urlhealth::check_urls(&urls).await
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn create_url(project: String, label: String, url: String, state: tauri::State<'_, AppState>) -> Result<storage::Url, String> {
    create_url_pour_hote(&state, project, label, url)
}

/// La logique de `create_url`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn create_url_pour_hote(state: &AppState, project: String, label: String, url: String) -> Result<storage::Url, String> {
    state.db.create_url(&project, &label, &url)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn update_url(id: i64, label: String, url: String, state: tauri::State<'_, AppState>) -> Result<storage::Url, String> {
    update_url_pour_hote(&state, id, label, url)
}

/// La logique de `update_url`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn update_url_pour_hote(state: &AppState, id: i64, label: String, url: String) -> Result<storage::Url, String> {
    state.db.update_url(id, &label, &url)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn delete_url(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_url_pour_hote(&state, id)
}

/// La logique de `delete_url`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn delete_url_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_url(id)
}

// --- Tauri Commands: Commandes rapides par projet ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_project_commands(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::ProjectCommand>, String> {
    get_project_commands_pour_hote(&state, project)
}

/// La logique de `get_project_commands`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_project_commands_pour_hote(state: &AppState, project: String) -> Result<Vec<storage::ProjectCommand>, String> {
    state.db.get_project_commands(&project)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn create_project_command(project: String, label: String, command: String, state: tauri::State<'_, AppState>) -> Result<storage::ProjectCommand, String> {
    create_project_command_pour_hote(&state, project, label, command)
}

/// La logique de `create_project_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn create_project_command_pour_hote(state: &AppState, project: String, label: String, command: String) -> Result<storage::ProjectCommand, String> {
    state.db.create_project_command(&project, &label, &command)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn update_project_command(id: i64, label: String, command: String, state: tauri::State<'_, AppState>) -> Result<storage::ProjectCommand, String> {
    update_project_command_pour_hote(&state, id, label, command)
}

/// La logique de `update_project_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn update_project_command_pour_hote(state: &AppState, id: i64, label: String, command: String) -> Result<storage::ProjectCommand, String> {
    state.db.update_project_command(id, &label, &command)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn delete_project_command(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_project_command_pour_hote(&state, id)
}

/// La logique de `delete_project_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn delete_project_command_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_project_command(id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn reorder_project_commands(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    reorder_project_commands_pour_hote(&state, ids)
}

/// La logique de `reorder_project_commands`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn reorder_project_commands_pour_hote(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_project_commands(&ids)
}

// --- Tauri Commands: Project Folders ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_project_folders(state: tauri::State<'_, AppState>) -> Result<Vec<storage::ProjectFolder>, String> {
    get_project_folders_pour_hote(&state)
}

/// La logique de `get_project_folders`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_project_folders_pour_hote(state: &AppState) -> Result<Vec<storage::ProjectFolder>, String> {
    state.db.get_project_folders()
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn create_project_folder(name: String, parent_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<storage::ProjectFolder, String> {
    create_project_folder_pour_hote(&state, name, parent_id)
}

/// La logique de `create_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn create_project_folder_pour_hote(state: &AppState, name: String, parent_id: Option<i64>) -> Result<storage::ProjectFolder, String> {
    state.db.create_project_folder(&name, parent_id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn rename_project_folder(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    rename_project_folder_pour_hote(&state, id, name)
}

/// La logique de `rename_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn rename_project_folder_pour_hote(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.db.rename_project_folder(id, &name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn delete_project_folder(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_project_folder_pour_hote(&state, id)
}

/// La logique de `delete_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn delete_project_folder_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_project_folder(id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn reorder_project_folders(ids: Vec<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    reorder_project_folders_pour_hote(&state, ids)
}

/// La logique de `reorder_project_folders`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn reorder_project_folders_pour_hote(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_project_folders(&ids)
}

/// Deplace un dossier sous un autre (`parent_id` a None = racine). Refuse les boucles.
#[cfg(feature = "interface-tauri")]
#[tauri::command]
fn move_project_folder(id: i64, parent_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    move_project_folder_pour_hote(&state, id, parent_id)
}

/// La logique de `move_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn move_project_folder_pour_hote(state: &AppState, id: i64, parent_id: Option<i64>) -> Result<(), String> {
    state.db.move_project_folder(id, parent_id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn move_project_to_folder(project_name: String, folder_id: Option<i64>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    move_project_to_folder_pour_hote(&state, project_name, folder_id)
}

/// La logique de `move_project_to_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn move_project_to_folder_pour_hote(state: &AppState, project_name: String, folder_id: Option<i64>) -> Result<(), String> {
    state.db.move_project_to_folder(&project_name, folder_id)
}

// --- Tauri Commands: Scanner ---

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn scan_dir(path: String) -> Result<scanner::ScanResult, String> {
    scanner::scan(&path)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn scan_subdirs(path: String) -> Result<Vec<scanner::ScanResult>, String> {
    scanner::scan_subdirs(&path)
}

// --- Tauri Commands: Settings (DB projects) ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_db_projects(state: tauri::State<'_, AppState>) -> Result<Vec<storage::Project>, String> {
    get_db_projects_pour_hote(&state)
}

/// La logique de `get_db_projects`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_db_projects_pour_hote(state: &AppState) -> Result<Vec<storage::Project>, String> {
    state.db.get_projects()
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn add_project(
    name: String,
    path: String,
    compose_file: String,
    description: String,
    depends_on: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<storage::Project, String> {
    add_project_pour_hote(&state, name, path, compose_file, description, depends_on).await
}

/// La logique de `add_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn add_project_pour_hote(state: &AppState, name: String, path: String, compose_file: String, description: String, depends_on: Vec<String>) -> Result<storage::Project, String> {
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

#[cfg(feature = "interface-tauri")]

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
    update_db_project_pour_hote(&state, id, name, path, compose_file, description, depends_on)
}

/// La logique de `update_db_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn update_db_project_pour_hote(state: &AppState, id: i64, name: String, path: String, compose_file: String, description: String, depends_on: Vec<String>) -> Result<storage::Project, String> {
    state.db.update_project(id, &name, &path, &compose_file, &description, &depends_on)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn delete_db_project(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_db_project_pour_hote(&state, id).await
}

/// La logique de `delete_db_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn delete_db_project_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
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

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn reorder_projects(names: Vec<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    reorder_projects_pour_hote(&state, names)
}

/// La logique de `reorder_projects`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn reorder_projects_pour_hote(state: &AppState, names: Vec<String>) -> Result<(), String> {
    state.db.reorder_projects(&names)
}

/// Nom DB reel d'un projet a partir de son nom AFFICHE (orchestrateur, en
/// memoire). Le nom affiche peut avoir derive du nom stocke (renommages) :
/// on retombe alors sur le chemin, identite stable — meme logique que
/// rename_project. Sans correspondance, retourne le nom affiche tel quel.
/// Prend `&AppState` et non `&State<..>` : elle est appelee depuis les commandes Tauri
/// comme depuis les fonctions `_pour_hote`, et seul le premier des deux connait Tauri.
async fn resolve_db_project_name(state: &AppState, display_name: &str) -> String {
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

/// Ce que Cockpit a trouve comme fichier compose dans un projet.
///
/// Le frontend n'invente rien : il AFFICHE ce que la detection a decide, et ne propose de
/// changer que parmi ce qui existe reellement sur le disque. C'est ce qui remplace le champ ou
/// l'on saisissait un chemin — un chemin saisi peut etre faux, une liste detectee non.
#[derive(serde::Serialize)]
struct ComposeDetecte {
    /// Le fichier utilise, relatif au dossier du projet. Vide si le projet n'en a aucun.
    retenu: String,
    /// Tout ce qui a ete trouve, le plus probable d'abord.
    candidats: Vec<String>,
    /// Le fichier vient-il d'un choix de l'utilisateur, ou de la detection seule ?
    choisi_a_la_main: bool,
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn docker_compose_detecte(
    name: String,
    rafraichir: bool,
    state: tauri::State<'_, AppState>,
) -> Result<ComposeDetecte, String> {
    docker_compose_detecte_pour_hote(&state, name, rafraichir).await
}

/// La logique de `docker_compose_detecte`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn docker_compose_detecte_pour_hote(state: &AppState, name: String, rafraichir: bool) -> Result<ComposeDetecte, String> {
    let db_name = resolve_db_project_name(&state, &name).await;
    let projet = state.db.get_project_by_name(&db_name)?;
    let racine = std::path::PathBuf::from(&projet.path);

    // Un geste explicite de l'utilisateur doit voir l'etat du disque MAINTENANT, pas celui
    // d'il y a dix secondes : un fichier compose qu'on vient de creer apparait tout de suite.
    if rafraichir {
        docker::detection::oublier(&racine);
    }

    let candidats = docker::detection::candidats(&racine);
    let choisi = !projet.compose_file.is_empty()
        && racine.join(&projet.compose_file).is_file();
    let retenu = if choisi {
        projet.compose_file.clone()
    } else {
        candidats.first().cloned().unwrap_or_default()
    };
    Ok(ComposeDetecte { retenu, candidats, choisi_a_la_main: choisi })
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn get_project_settings(name: String, state: tauri::State<'_, AppState>) -> Result<storage::Project, String> {
    get_project_settings_pour_hote(&state, name).await
}

/// La logique de `get_project_settings`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn get_project_settings_pour_hote(state: &AppState, name: String) -> Result<storage::Project, String> {
    let db_name = resolve_db_project_name(&state, &name).await;
    state.db.get_project_by_name(&db_name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn update_project_settings(
    name: String,
    path: String,
    compose_file: String,
    description: String,
    depends_on: Vec<String>,
    state: tauri::State<'_, AppState>,
) -> Result<storage::Project, String> {
    update_project_settings_pour_hote(&state, name, path, compose_file, description, depends_on).await
}

/// La logique de `update_project_settings`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn update_project_settings_pour_hote(state: &AppState, name: String, path: String, compose_file: String, description: String, depends_on: Vec<String>) -> Result<storage::Project, String> {
    let db_name = resolve_db_project_name(&state, &name).await;
    let proj = state.db.update_project_by_name(&db_name, &path, &compose_file, &description, &depends_on)?;
    // L'orchestrateur est indexe par le nom AFFICHE, lui
    state.orchestrator.update_project(&name, &path, &compose_file, &description, depends_on).await;
    Ok(proj)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn rename_project(
    old_name: String,
    new_name: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    rename_project_pour_hote(&state, old_name, new_name).await
}

/// La logique de `rename_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn rename_project_pour_hote(state: &AppState, old_name: String, new_name: String) -> Result<(), String> {
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

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn get_system_metrics(
    state: tauri::State<'_, AppState>,
) -> Result<system::metrics::SystemMetrics, String> {
    get_system_metrics_pour_hote(&state).await
}

/// La logique de `get_system_metrics`, appelable par TOUT hote.
pub async fn get_system_metrics_pour_hote(
    state: &AppState,
) -> Result<system::metrics::SystemMetrics, String> {
    let mut collector = state.collector.lock().await;
    Ok(collector.collect())
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn kill_process(pid: u32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    kill_process_pour_hote(&state, pid).await
}

/// La logique de `kill_process`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn kill_process_pour_hote(state: &AppState, pid: u32) -> Result<(), String> {
    let collector = state.collector.lock().await;
    system::process::kill_process_with_sys(collector.system(), pid)
}

// --- Tauri Commands: Apparence (image de fond) ---

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn set_wallpaper(data_url: String) -> Result<(), String> {
    set_wallpaper_pour_hote(data_url)
}

/// La logique de `set_wallpaper`. L'`AppHandle` n'y servait qu'a trouver le dossier de donnees,
/// que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
fn set_wallpaper_pour_hote(data_url: String) -> Result<(), String> {
    let dossier = crate::chemins::dossier_donnees().ok_or("dossier de donnees inconnu")?;
    appearance::set_wallpaper(dossier, &data_url)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn get_wallpaper() -> Result<Option<String>, String> {
    get_wallpaper_pour_hote()
}

/// La logique de `get_wallpaper`. L'`AppHandle` n'y servait qu'a trouver le dossier de donnees,
/// que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
fn get_wallpaper_pour_hote() -> Result<Option<String>, String> {
    let dossier = crate::chemins::dossier_donnees().ok_or("dossier de donnees inconnu")?;
    appearance::get_wallpaper(dossier)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn clear_wallpaper() -> Result<(), String> {
    clear_wallpaper_pour_hote()
}

/// La logique de `clear_wallpaper`. L'`AppHandle` n'y servait qu'a trouver le dossier de donnees,
/// que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
fn clear_wallpaper_pour_hote() -> Result<(), String> {
    let dossier = crate::chemins::dossier_donnees().ok_or("dossier de donnees inconnu")?;
    appearance::clear_wallpaper(dossier)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn read_image_as_data_url(path: String) -> Result<String, String> {
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
#[cfg(feature = "interface-tauri")]
#[tauri::command]
fn set_webview_zoom(window: tauri::WebviewWindow, factor: f64) -> Result<(), String> {
    if !(ZOOM_MIN..=ZOOM_MAX).contains(&factor) {
        return Err(format!("facteur de zoom hors bornes : {}", factor));
    }
    window.set_zoom(factor).map_err(|e| e.to_string())
}

// --- Tauri Command: Import DB ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn import_database(path: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    import_database_pour_hote(&state, path).await
}

/// La logique de `import_database`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn import_database_pour_hote(state: &AppState, path: String) -> Result<String, String> {
    storage::import::import_from(&state.db, &path)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_db_path(state: tauri::State<'_, AppState>) -> String {
    get_db_path_pour_hote(&state)
}

/// La logique de `get_db_path`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_db_path_pour_hote(state: &AppState) -> String {
    state.db_path.clone()
}

// --- Tauri Commands: Enregistrement de reunions ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn start_recording(
    project: String,
    state: tauri::State<'_, AppState>,
) -> Result<recorder::RecordingStatus, String> {
    start_recording_pour_hote(&state, project).await
}

/// La logique de `start_recording`. L'`AppHandle` n'y servait qu'a emettre l'etat de
/// l'enregistrement : l'emetteur de l'etat le fait, sans connaitre l'hote.
async fn start_recording_pour_hote(
    state: &AppState,
    project: String,
) -> Result<recorder::RecordingStatus, String> {
    recorder::start(state.emetteur.clone(), state.db.clone(), &state.recorder, project).await
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn stop_recording(state: tauri::State<'_, AppState>) -> Result<(), String> {
    stop_recording_pour_hote(&state).await
}

/// La logique de `stop_recording`.
async fn stop_recording_pour_hote(state: &AppState) -> Result<(), String> {
    recorder::stop(state.emetteur.clone(), state.db.clone(), &state.recorder).await
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_active_recording(state: tauri::State<'_, AppState>) -> Option<recorder::RecordingStatus> {
    get_active_recording_pour_hote(&state)
}

/// La logique de `get_active_recording`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_active_recording_pour_hote(state: &AppState) -> Option<recorder::RecordingStatus> {
    recorder::active_status(&state.recorder)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_failed_recordings(project: String, state: tauri::State<'_, AppState>) -> Result<Vec<storage::Recording>, String> {
    get_failed_recordings_pour_hote(&state, project)
}

/// La logique de `get_failed_recordings`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn get_failed_recordings_pour_hote(state: &AppState, project: String) -> Result<Vec<storage::Recording>, String> {
    state.db.get_failed_recordings(&project)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn retry_recording(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    retry_recording_pour_hote(&state, id)
}

/// La logique de `retry_recording`.
fn retry_recording_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    recorder::retry(state.emetteur.clone(), state.db.clone(), id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn delete_recording(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    delete_recording_pour_hote(&state, id)
}

/// La logique de `delete_recording`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn delete_recording_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    recorder::delete(&state.db, id)
}

// --- Tauri Commands: App settings (cle API, prompt de resume) ---

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn get_app_settings(
    state: tauri::State<'_, AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    get_app_settings_pour_hote(&state)
}

/// La logique de `get_app_settings`, appelable par TOUT hote.
pub fn get_app_settings_pour_hote(
    state: &AppState,
) -> Result<std::collections::HashMap<String, String>, String> {
    let mut settings = state.db.get_all_settings()?;
    settings
        .entry("summary_prompt".into())
        .or_insert_with(|| recorder::summarize::DEFAULT_PROMPT.to_string());
    // Le modele par defaut appartient au fournisseur qui redigera : `gpt-4o` ne veut rien dire
    // ailleurs que chez OpenAI. Aucun fournisseur capable = aucun defaut a proposer, et
    // l'ecran des reunions le dit deja.
    if let Some((_, modele)) = llm::pour(&state.db, |f| f.texte()) {
        settings
            .entry("summary_model".into())
            .or_insert_with(|| modele.modele_par_defaut().to_string());
    }
    Ok(settings)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn set_app_setting(key: String, value: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    set_app_setting_pour_hote(&state, key, value)
}

/// La logique de `set_app_setting`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn set_app_setting_pour_hote(state: &AppState, key: String, value: String) -> Result<(), String> {
    state.db.set_setting(&key, &value)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn get_project_summary_prompt(project: String, state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    get_project_summary_prompt_pour_hote(&state, project).await
}

/// La logique de `get_project_summary_prompt`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn get_project_summary_prompt_pour_hote(state: &AppState, project: String) -> Result<Option<String>, String> {
    let db_name = resolve_db_project_name(&state, &project).await;
    state.db.get_project_summary_prompt(&db_name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn set_project_summary_prompt(project: String, prompt: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    set_project_summary_prompt_pour_hote(&state, project, prompt).await
}

/// La logique de `set_project_summary_prompt`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn set_project_summary_prompt_pour_hote(state: &AppState, project: String, prompt: Option<String>) -> Result<(), String> {
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
// que `write_terminal` : c'est le chemin de frappe. La commande depose maintenant la trame dans
// la file du fil d'ecriture du client, donc elle ne bloque pas sur le socket.
//
// **LE DISQUE COMPTE AUSSI, ET CA A ETE OUBLIE UNE FOIS.** Dix-huit commandes lisaient ou
// ecrivaient des fichiers sans `async` : un simple `stat` est instantane sur un disque local,
// et ne revient JAMAIS sur un montage reseau ou FUSE decroche — la fenetre gele alors sans
// autre issue que de tuer l'application. Une commande qui touche un chemin fourni par
// l'utilisateur est `async fn`, sans exception.

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn create_terminal(
    project: String,
    cwd: String,
    cols: u16,
    rows: u16,
    init_command: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<i64, String> {
    create_terminal_pour_hote(&state, project, cwd, cols, rows, init_command).await
}

/// La logique de `create_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn create_terminal_pour_hote(state: &AppState, project: String, cwd: String, cols: u16, rows: u16, init_command: Option<String>) -> Result<i64, String> {
    let demande = terminal::Creation {
        projet: project,
        dossier: cwd,
        taille: terminal::Taille { colonnes: cols, lignes: rows },
        commande_initiale: init_command,
    };
    state.terminals.creer(&state.db, demande)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn write_terminal(id: i64, data: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    write_terminal_pour_hote(&state, id, data)
}

/// La logique de `write_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn write_terminal_pour_hote(state: &AppState, id: i64, data: String) -> Result<(), String> {
    state.terminals.ecrire(id, &data)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn resize_terminal(id: i64, cols: u16, rows: u16, state: tauri::State<'_, AppState>) -> Result<(), String> {
    resize_terminal_pour_hote(&state, id, cols, rows).await
}

/// La logique de `resize_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn resize_terminal_pour_hote(state: &AppState, id: i64, cols: u16, rows: u16) -> Result<(), String> {
    state
        .terminals
        .redimensionner(&state.db, id, terminal::Taille { colonnes: cols, lignes: rows })
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn close_terminal(id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    close_terminal_pour_hote(&state, id).await
}

/// La logique de `close_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn close_terminal_pour_hote(state: &AppState, id: i64) -> Result<(), String> {
    state.terminals.fermer(&state.db, id)
}

/// Photographie les terminaux ouverts pour qu'ils reviennent « comme on les a quittes ».
///
/// Appelee par l'interface quand on quitte la vue des terminaux — pas sur un minuteur : le
/// cout se paie par terminal, et l'implementation refuse de recommencer avant une minute. La
/// fenetre qui se ferme declenche la meme chose, mais sans borne (voir `fenetre.rs`).
#[cfg(feature = "interface-tauri")]
#[tauri::command]
async fn save_terminal_screens(state: tauri::State<'_, AppState>) -> Result<(), String> {
    save_terminal_screens_pour_hote(&state).await
}

/// La logique de `save_terminal_screens`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn save_terminal_screens_pour_hote(state: &AppState) -> Result<(), String> {
    state.terminals.enregistrer_les_ecrans(&state.db, false);
    Ok(())
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn attach_terminal(
    id: i64,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    attach_terminal_pour_hote(&state, id, cols, rows).await
}

/// La logique de `attach_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn attach_terminal_pour_hote(state: &AppState, id: i64, cols: u16, rows: u16) -> Result<(), String> {
    state
        .terminals
        .attacher(&state.db, id, terminal::Taille { colonnes: cols, lignes: rows })
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn rename_terminal(id: i64, name: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    rename_terminal_pour_hote(&state, id, name)
}

/// La logique de `rename_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn rename_terminal_pour_hote(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.terminals.renommer(&state.db, id, &name)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn list_terminals(
    project: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<terminal::TerminalInfo>, String> {
    list_terminals_pour_hote(&state, project).await
}

/// La logique de `list_terminals`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn list_terminals_pour_hote(state: &AppState, project: String) -> Result<Vec<terminal::TerminalInfo>, String> {
    Ok(state.terminals.lister(&state.db, Some(&project)))
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn list_all_terminals(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<terminal::TerminalInfo>, String> {
    list_all_terminals_pour_hote(&state).await
}

/// La logique de `list_all_terminals`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn list_all_terminals_pour_hote(state: &AppState) -> Result<Vec<terminal::TerminalInfo>, String> {
    Ok(state.terminals.lister(&state.db, None))
}

/// Presse-papier systeme. Instance arboard gardee en vie : sous X11 le contenu
/// du presse-papier disparait quand son proprietaire (la connexion) est droppe.
static CLIPBOARD: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);

#[cfg_attr(feature = "interface-tauri", tauri::command)]
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

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn get_clipboard() -> Result<String, String> {
    let mut guard = CLIPBOARD.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    // Presse-papier vide = chaine vide (pas une erreur)
    Ok(guard.as_mut().unwrap().get_text().unwrap_or_default())
}

// --- Tauri Commands: les fournisseurs d'IA ---
//
// Aucune de ces commandes ne nomme un produit : elles s'adressent au fournisseur choisi dans
// les reglages. C'est ce qui permet d'en declarer un nouveau sans toucher a l'interface.

/// Le catalogue et ce que chacun sait faire. Le frontend s'en sert pour n'afficher que ce qui
/// existe : un bouton qui promet ce que le fournisseur ne sait pas faire est un mensonge.
#[cfg(feature = "interface-tauri")]
#[tauri::command]
fn llm_catalogue(state: tauri::State<'_, AppState>) -> Vec<llm::Capacites> {
    llm_catalogue_pour_hote(&state)
}

/// La logique de `llm_catalogue`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_catalogue_pour_hote(state: &AppState) -> Vec<llm::Capacites> {
    llm::catalogue_pour_le_frontend(&state.db)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_choisir(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    llm_choisir_pour_hote(&state, id)
}

/// La logique de `llm_choisir`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_choisir_pour_hote(state: &AppState, id: String) -> Result<(), String> {
    llm::choisir(&state.db, &id)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_poser_cle(id: String, cle: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    llm_poser_cle_pour_hote(&state, id, cle)
}

/// La logique de `llm_poser_cle`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_poser_cle_pour_hote(state: &AppState, id: String, cle: String) -> Result<(), String> {
    llm::poser_cle_api(&state.db, &id, &cle)
}

/// Les conversations passees du projet, chez le fournisseur choisi.
#[cfg(feature = "interface-tauri")]
#[tauri::command]
fn llm_conversations(
    project_path: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<llm::Conversation>, String> {
    llm_conversations_pour_hote(&state, project_path)
}

/// La logique de `llm_conversations`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_conversations_pour_hote(state: &AppState, project_path: String) -> Result<Vec<llm::Conversation>, String> {
    llm::conversations::lister(&state.db, llm::prefere(&state.db), &project_path)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_renommer_conversation(
    conversation_id: String,
    nom: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    llm_renommer_conversation_pour_hote(&state, conversation_id, nom)
}

/// La logique de `llm_renommer_conversation`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_renommer_conversation_pour_hote(state: &AppState, conversation_id: String, nom: String) -> Result<(), String> {
    llm::conversations::renommer(
        &state.db,
        llm::prefere(&state.db).id(),
        &conversation_id,
        &nom,
    )
}

/// Les commandes de terminal du fournisseur choisi : reprendre une conversation, en ouvrir une
/// neuve. **Elles viennent de lui** : `--resume` ne veut rien dire ailleurs.
#[derive(serde::Serialize)]
struct CommandesAgent {
    neuve: String,
    reprise: Option<String>,
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_commandes(
    conversation_id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<CommandesAgent, String> {
    llm_commandes_pour_hote(&state, conversation_id)
}

/// La logique de `llm_commandes`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_commandes_pour_hote(state: &AppState, conversation_id: Option<String>) -> Result<CommandesAgent, String> {
    let fournisseur = llm::prefere(&state.db);
    let lecteur = fournisseur
        .conversations()
        .ok_or_else(|| format!("{} n'a pas de conversations a reprendre", fournisseur.nom()))?;
    Ok(CommandesAgent {
        neuve: lecteur.commande_neuve(),
        reprise: conversation_id.map(|id| lecteur.commande_de_reprise(&id)),
    })
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn record_command(project: String, command: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    record_command_pour_hote(&state, project, command)
}

/// La logique de `record_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn record_command_pour_hote(state: &AppState, project: String, command: String) -> Result<(), String> {
    terminal::history::record(&state.db, &project, &command)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn terminal_search(
    id: i64,
    action: String,
    query: String,
    state: tauri::State<'_, AppState>,
) -> Result<terminal::ResultatRecherche, String> {
    terminal_search_pour_hote(&state, id, action, query).await
}

/// La logique de `terminal_search`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn terminal_search_pour_hote(state: &AppState, id: i64, action: String, query: String) -> Result<terminal::ResultatRecherche, String> {
    let action = terminal::ActionRecherche::depuis_texte(&action)?;
    state.terminals.chercher(&state.db, id, action, &query)
}

/// Qui transcrira et qui redigera le prochain compte rendu de reunion.
///
/// **La regle vit cote Rust, une seule fois** : le fournisseur choisi s'il sait faire, sinon le
/// premier capable et configure. La recopier dans l'interface donnerait deux verites pour une
/// meme decision, et l'ecran finirait par annoncer un fournisseur pendant qu'un autre travaille.
#[derive(serde::Serialize)]
struct AffectationsReunion {
    transcription: Option<String>,
    redaction: Option<String>,
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_reunions(state: tauri::State<'_, AppState>) -> AffectationsReunion {
    llm_reunions_pour_hote(&state)
}

/// La logique de `llm_reunions`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_reunions_pour_hote(state: &AppState) -> AffectationsReunion {
    AffectationsReunion {
        transcription: llm::pour(&state.db, |f| f.transcription()).map(|(f, _)| f.nom().to_string()),
        redaction: llm::pour(&state.db, |f| f.texte()).map(|(f, _)| f.nom().to_string()),
    }
}

// --- Tauri Commands: abonnement d'un fournisseur ---

/// L'etat de connexion du fournisseur donne, ou du fournisseur choisi.
#[cfg(feature = "interface-tauri")]
#[tauri::command]
fn llm_abonnement(
    id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<llm::EtatAbonnement, String> {
    llm_abonnement_pour_hote(&state, id)
}

/// La logique de `llm_abonnement`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_abonnement_pour_hote(state: &AppState, id: Option<String>) -> Result<llm::EtatAbonnement, String> {
    let fournisseur = match id {
        Some(id) => llm::par_id(&id).ok_or_else(|| format!("fournisseur inconnu : {id}"))?,
        None => llm::prefere(&state.db),
    };
    Ok(llm::abonnement::etat(fournisseur))
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_connexion_demarrer(
    id: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    llm_connexion_demarrer_pour_hote(&state, id)
}

/// La logique de `llm_connexion_demarrer`. L'`AppHandle` n'y servait qu'a emettre la
/// sortie de la connexion guidee : l'emetteur de l'etat le fait, sans connaitre l'hote.
fn llm_connexion_demarrer_pour_hote(
    state: &AppState,
    id: Option<String>,
) -> Result<(), String> {
    let fournisseur = match id {
        Some(id) => llm::par_id(&id).ok_or_else(|| format!("fournisseur inconnu : {id}"))?,
        None => llm::prefere(&state.db),
    };
    state.connexion_llm.demarrer(state.emetteur.clone(), fournisseur)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_connexion_entrer(data: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    llm_connexion_entrer_pour_hote(&state, data)
}

/// La logique de `llm_connexion_entrer`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_connexion_entrer_pour_hote(state: &AppState, data: String) -> Result<(), String> {
    state.connexion_llm.entrer(&data)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn llm_connexion_annuler(state: tauri::State<'_, AppState>) {
    llm_connexion_annuler_pour_hote(&state)
}

/// La logique de `llm_connexion_annuler`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn llm_connexion_annuler_pour_hote(state: &AppState)  {
    state.connexion_llm.annuler()
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

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn open_url(url: String) -> Result<(), String> {
    if !schema_ouvrable(&url) {
        return Err(format!("adresse non ouvrable : {url}"));
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
/// Journalise une erreur et la remonte au serveur de suivi si l'utilisateur l'a accepte.
///
/// Le JOURNAL est ecrit dans tous les cas : il marche hors ligne, ne demande aucun accord,
/// et c'est lui qu'on relit quand une erreur est signalee de vive voix. L'ENVOI, lui, est
/// conditionne a l'accord explicite, et refuse un transport en clair (voir report::send).
///
/// N'echoue jamais : une remontee cassee ne doit pas ajouter une erreur a l'erreur.
async fn report_error(
    scope: String,
    message: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    report_error_pour_hote(&state, scope, message).await
}

/// La logique de `report_error`, appelable par tout hote.
///
/// **L'`AppHandle` A DISPARU DE LA SIGNATURE.** Il n'y servait qu'a trouver le dossier de
/// donnees, que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
/// C'est le meme decouplage que celui du journal des terminaux : garder le handle pour ca
/// rendait la commande inutilisable ailleurs sans aucune raison.
async fn report_error_pour_hote(
    state: &AppState,
    scope: String,
    message: String,
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

    if let Some(dir) = crate::chemins::dossier_donnees() {
        let horodatage = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        report::append_log(dir, &report::format_log_line(&horodatage, &scope, &message));
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

#[cfg_attr(feature = "interface-tauri", tauri::command)]
/// Fiche technique de la machine, telle qu'elle accompagne les erreurs.
/// Sert aussi a l'afficher a l'utilisateur : il doit pouvoir voir ce qui serait envoye.
///
/// `async fn` et non `fn` : la fiche interroge le serveur audio pour dire quel appareil
/// serait retenu, et une commande sans `async` s'execute EN LIGNE dans la boucle
/// principale GTK — l'interface ne repeint plus pendant ce temps. C'etait deja vrai
/// avant, avec deux `Command::new` (`pactl`, `pw-record`) au meme endroit.
async fn machine_report() -> report::MachineInfo {
    taches::lancer_bloquant(|| report::machine_info().clone())
        .await
        .unwrap_or_else(|_| report::machine_info().clone())
}

/// La page rend compte de sa sante : a-t-elle peint depuis son dernier passage, la fenetre
/// etait-elle visible et concentree, et l'utilisateur a-t-il touche le clavier ou la souris
/// recemment. « Pas peint » avec un appel qui arrive quand meme veut dire que le JavaScript
/// tourne et que rien n'est dessine. Le focus disculpe la fenetre recouverte, qui reste
/// « visible » sans qu'on la regarde. L'entree recente dit si quelqu'un est devant : le
/// guetteur ne recharge la vue que dans ce cas — voir `guetteur`.
#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn sante_page(a_peint: bool, visible: bool, concentre: bool, entree_recente: bool) {
    guetteur::signe_de_la_page(a_peint, visible, concentre, entree_recente);
}

/// Le mode secours du rendu : disponible sous Linux seulement, et deja active ou non.
#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn mode_secours_rendu() -> rendu::EtatModeSecours {
    rendu::etat_mode_secours()
}

/// Pose ou retire le mode secours du rendu. Le changement prend effet au prochain lancement :
/// la variable de WebKitGTK se lit AVANT l'initialisation de GTK, donc avant que cette
/// commande puisse exister dans le processus en cours.
#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn activer_mode_secours_rendu(activer: bool) -> Result<(), String> {
    rendu::basculer_mode_secours(activer)
}

/// Relance l'application : la nouvelle instance est lancee, puis celle-ci s'arrete.
///
/// `async fn` car elle lance un processus externe. Le chemin est celui du guetteur : il
/// libere le nom single-instance AVANT de lancer (sinon la nouvelle instance se tue en le
/// trouvant pris), choisit `$APPIMAGE` sous AppImage, et journalise. La mise a jour, elle,
/// reste sur le `relaunch()` du plugin process : son flux est verifie de bout en bout et
/// l'AppImage le met a l'abri de la course (sa nouvelle instance demarre lentement).
#[cfg(feature = "interface-tauri")]
#[tauri::command]
async fn relancer_application(app: tauri::AppHandle) -> Result<(), String> {
    guetteur::relancer_l_application(&app)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn debug_log(line: String) {
    use std::io::Write;
    // `temp_dir()` et pas `/tmp` : sous Windows le dossier temporaire est dans le profil de
    // l'utilisateur, et un chemin absolu `/tmp` y designerait la racine du lecteur courant.
    let chemin = std::env::temp_dir().join("cockpit-debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(chemin) {
        let _ = writeln!(f, "{}", line);
    }
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
fn search_command_history(
    query: String,
    limit: Option<usize>,
    state: tauri::State<'_, AppState>,
) -> Vec<terminal::history::HistoryEntry> {
    search_command_history_pour_hote(&state, query, limit)
}

/// La logique de `search_command_history`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
fn search_command_history_pour_hote(state: &AppState, query: String, limit: Option<usize>) -> Vec<terminal::history::HistoryEntry> {
    terminal::history::search(&state.db, &query, limit.unwrap_or(50))
}

// --- Tauri Commands: Explorateur de fichiers ---

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn list_project_dir(project_path: String, rel_path: String) -> Result<Vec<workspace::DirEntry>, String> {
    workspace::list_dir(&project_path, &rel_path)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn read_project_file(project_path: String, rel_path: String) -> Result<workspace::FileContent, String> {
    workspace::read_project_file(&project_path, &rel_path)
}

/// Etat disque du fichier affiche : sert au suivi des modifications exterieures
/// (relire 2 Mo toutes les deux secondes serait absurde, un stat ne coute rien).
#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn stat_project_file(
    project_path: String,
    rel_path: String,
) -> Result<Option<workspace::FileStat>, String> {
    workspace::stat_project_file(&project_path, &rel_path)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn write_project_file(project_path: String, rel_path: String, content: String) -> Result<(), String> {
    workspace::write_project_file(&project_path, &rel_path, &content)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn read_project_image(project_path: String, rel_path: String) -> Result<String, String> {
    workspace::read_project_image(&project_path, &rel_path)
}

#[cfg(feature = "interface-tauri")]

#[tauri::command]
async fn backup_database(dest: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    backup_database_pour_hote(&state, dest).await
}

/// La logique de `backup_database`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn backup_database_pour_hote(state: &AppState, dest: String) -> Result<(), String> {
    state.db.backup_to(&dest)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn create_project_file(project_path: String, rel_dir: String, name: String) -> Result<String, String> {
    workspace::create_project_file(&project_path, &rel_dir, &name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn create_project_dir(project_path: String, rel_dir: String, name: String) -> Result<String, String> {
    workspace::create_project_dir(&project_path, &rel_dir, &name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn rename_project_entry(project_path: String, rel_path: String, new_name: String) -> Result<String, String> {
    workspace::rename_project_entry(&project_path, &rel_path, &new_name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn trash_project_entry(project_path: String, rel_path: String) -> Result<(), String> {
    workspace::trash_project_entry(&project_path, &rel_path)
}

// async : la recherche parcourt tout le projet, elle ne doit pas bloquer le thread principal
#[cfg_attr(feature = "interface-tauri", tauri::command)]
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
#[cfg(feature = "interface-tauri")]
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
    goto_definition_pour_hote(&state, project_path, lang, rel_path, content, line, character, symbol).await
}

/// La logique de `goto_definition`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
async fn goto_definition_pour_hote(state: &AppState, project_path: String, lang: String, rel_path: String, content: String, line: u32, character: u32, symbol: String) -> Result<GotoDefinitionResult, String> {
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

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_status(project_path: String) -> Result<gitdiff::GitStatus, String> {
    gitdiff::git_status(&project_path).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_diff_file(
    project_path: String,
    path: String,
    untracked: bool,
) -> Result<gitdiff::FileDiff, String> {
    gitdiff::git_diff_file(&project_path, &path, untracked).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_stage(project_path: String, path: String) -> Result<(), String> {
    gitdiff::git_stage(&project_path, &path).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_unstage(project_path: String, path: String) -> Result<(), String> {
    gitdiff::git_unstage(&project_path, &path).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_stage_all(project_path: String) -> Result<(), String> {
    gitdiff::git_stage_all(&project_path).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_unstage_all(project_path: String) -> Result<(), String> {
    gitdiff::git_unstage_all(&project_path).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_commit(project_path: String, message: String) -> Result<(), String> {
    gitdiff::git_commit(&project_path, &message).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_push(project_path: String, set_upstream: bool) -> Result<String, String> {
    gitdiff::git_push(&project_path, set_upstream).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_pull(project_path: String) -> Result<String, String> {
    gitdiff::git_pull(&project_path).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_log(project_path: String, limit: u32) -> Result<Vec<gitdiff::CommitInfo>, String> {
    gitdiff::git_log(&project_path, limit).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_commit_diff(project_path: String, hash: String) -> Result<Vec<gitdiff::FileDiff>, String> {
    gitdiff::git_commit_diff(&project_path, &hash).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_branches(project_path: String) -> Result<Vec<gitdiff::BranchInfo>, String> {
    gitdiff::git_branches(&project_path).await
}

/// Les dossiers de travail du depot, le principal en premier.
#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_worktrees(project_path: String) -> Result<Vec<gitdiff::worktree::Worktree>, String> {
    gitdiff::worktree::lister(&project_path).await
}

/// Ajoute un dossier de travail sur `branche`, en la creant si `creer`. Rend son chemin.
#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_worktree_add(
    project_path: String,
    branche: String,
    creer: bool,
) -> Result<String, String> {
    gitdiff::worktree::ajouter(&project_path, &branche, creer).await
}

/// Retire un dossier de travail. `force` abandonne les modifications non validees.
#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_worktree_remove(
    project_path: String,
    chemin: String,
    force: bool,
) -> Result<(), String> {
    gitdiff::worktree::retirer(&project_path, &chemin, force).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_checkout_branch(project_path: String, name: String) -> Result<(), String> {
    gitdiff::git_checkout_branch(&project_path, &name).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_create_branch(project_path: String, name: String) -> Result<(), String> {
    gitdiff::git_create_branch(&project_path, &name).await
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
async fn git_delete_branch(project_path: String, name: String, force: bool) -> Result<(), String> {
    gitdiff::git_delete_branch(&project_path, &name, force).await
}

// --- Tauri Commands: Agents marketplace (multi-marketplace) ---

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn get_marketplace_path() -> Result<String, String> {
    Ok(agents::ccm_marketplace_path()?.to_string_lossy().to_string())
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn list_marketplaces() -> Result<Vec<agents::MarketplaceLocation>, String> {
    agents::list_marketplaces()
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn list_plugins(marketplace_id: String) -> Result<Vec<agents::PluginInfo>, String> {
    agents::list_plugins_in(&marketplace_id)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn list_agents(marketplace_id: String, plugin: String) -> Result<Vec<agents::AgentInfo>, String> {
    agents::list_agents_in(&marketplace_id, &plugin)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn read_agent(marketplace_id: String, plugin: String, name: String) -> Result<String, String> {
    agents::read_agent(&marketplace_id, &plugin, &name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn save_agent(
    marketplace_id: String,
    plugin: String,
    name: String,
    content: String,
) -> Result<(), String> {
    agents::save_agent(&marketplace_id, &plugin, &name, &content)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn delete_agent(marketplace_id: String, plugin: String, name: String) -> Result<(), String> {
    agents::delete_agent(&marketplace_id, &plugin, &name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn rename_agent(
    marketplace_id: String,
    plugin: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    agents::rename_agent(&marketplace_id, &plugin, &old_name, &new_name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn create_plugin(name: String, description: String) -> Result<(), String> {
    agents::create_plugin(&name, &description)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn delete_plugin(marketplace_id: String, name: String) -> Result<(), String> {
    agents::delete_plugin(&marketplace_id, &name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn rename_plugin(
    marketplace_id: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    agents::rename_plugin(&marketplace_id, &old_name, &new_name)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn get_project_plugins(project_path: String) -> Result<Vec<String>, String> {
    agents::get_project_plugins(&project_path)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn set_project_plugins(project_path: String, plugins: Vec<String>) -> Result<(), String> {
    agents::set_project_plugins(&project_path, plugins)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn get_orchestrator_config() -> Result<agents::OrchestratorConfig, String> {
    agents::get_orchestrator_config()
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn set_teams_enabled(enabled: bool) -> Result<(), String> {
    agents::set_teams_enabled(enabled)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
fn set_teammate_mode(mode: String) -> Result<(), String> {
    agents::set_teammate_mode(&mode)
}

#[cfg_attr(feature = "interface-tauri", tauri::command)]
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

/// **CE LANCEUR N'EXISTE QU'AVEC TAURI.** Sans la feature, le binaire ne sert que le
/// pont : il n'ouvre aucune fenetre, et ne doit donc pas etre lie a WebKitGTK.
#[cfg(feature = "interface-tauri")]
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

    // Le mode de rendu de la vue web, pour la meme raison : WebKitGTK lit sa variable au
    // demarrage du moteur, donc AVANT le Builder. Voir `rendu` : le chemin DMA-BUF ne marche pas
    // de facon fiable avec le pilote proprietaire NVIDIA, et c'est la panne que le guetteur a
    // mesuree le 2026-08-31 — la page parle et ne peint plus rien.
    rendu::decider();

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

    // Ou chercher les programmes de l'utilisateur, calcule UNE fois et en tache de fond : le
    // PATH d'une application lancee depuis un menu de bureau ne contient pas `~/.local/bin`.
    commande::precharger_les_chemins();

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
        .on_window_event(fenetre::sur_evenement)
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
            let db_path = chemin_de_la_base(&app.path().app_data_dir().unwrap());

            log::info!("Using database: {}", db_path);
            let db = Database::new(&db_path)
                .expect("failed to open database");

            // Enregistrements restes en plein pipeline a la fermeture -> erreur (retry possible)
            let _ = db.fail_stale_recordings();

            // Le guetteur AVANT la mise en route des terminaux : c'est justement cette
            // mise en route qui peut prendre des secondes, et on veut qu'un gel la nomme.
            guetteur::surveiller(app.handle().clone());

            // Serveur de terminaux : mise en route (lancement du service s'il ne tourne
            // pas deja, puis reconciliation avec la base) avant toute autre operation.
            let terminaux = terminal::terminaux();
            let emetteur: crate::evenements::Emetteurs =
                std::sync::Arc::new(app.handle().clone());
            terminaux.preparer(emetteur.clone(), &db);

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

            // L'etat sort d'une fonction que TOUT hote peut appeler, pas du `setup` : c'est
            // ce qui permet de servir les memes commandes ailleurs que dans Tauri.
            let etat = construire_etat(db, db_path.clone(), terminaux, emetteur);
            let orchestrator = etat.orchestrator.clone();
            app.manage(etat);

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
            langue_imposee,
            // Compte et synchronisation
            compte::compte_etat,
            compte::compte_inscription,
            compte::compte_connexion,
            compte::compte_connexion_google,
            compte::compte_google_direct,
            compte::compte_appairage_demarrer,
            compte::compte_appairage_etat,
            compte::compte_deconnexion,
            compte::compte_definir_serveur,
            compte::compte_definir_nom,
            compte::compte_machines,
            compte::compte_google_disponible,
            compte::compte_deposer_avatar,
            compte::compte_lire_image,
            compte::compte_deposer_image,
            compte::compte_retirer_avatar,
            compte::synchro::synchro_maintenant,
            compte::synchro::synchro_etat,
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
            set_todo_progress,
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
            save_terminal_screens,
            rename_terminal,
            list_terminals,
            list_all_terminals,
            sante_page,
            mode_secours_rendu,
            activer_mode_secours_rendu,
            relancer_application,
            docker_compose_detecte,
            set_clipboard,
            get_clipboard,
            terminal_search,
            record_command,
            search_command_history,
            debug_log,
            report_error,
            machine_report,
            // Fournisseurs d'IA
            llm_catalogue,
            llm_choisir,
            llm_poser_cle,
            llm_conversations,
            llm_renommer_conversation,
            llm_commandes,
            llm_reunions,
            llm_abonnement,
            llm_connexion_demarrer,
            llm_connexion_entrer,
            llm_connexion_annuler,
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
            git_worktrees,
            git_worktree_add,
            git_worktree_remove,
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
    /// Une langue inconnue est traitee comme ABSENTE : une faute de frappe dans la variable ne
    /// doit pas faire demarrer l'interface dans une langue vide, ni faire echouer le harnais de
    /// captures sur un ecran a moitie traduit.
    #[test]
    fn seules_les_deux_langues_connues_sont_acceptees() {
        assert_eq!(super::langue_valide("fr").as_deref(), Some("fr"));
        assert_eq!(super::langue_valide("en").as_deref(), Some("en"));
        // Les espaces autour arrivent des qu'on pose la variable a la main.
        assert_eq!(super::langue_valide("  en \n").as_deref(), Some("en"));

        for inconnue in ["", "  ", "EN", "de", "fr-FR", "anglais"] {
            assert_eq!(
                super::langue_valide(inconnue),
                None,
                "« {inconnue} » ne doit pas etre acceptee",
            );
        }
    }

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
