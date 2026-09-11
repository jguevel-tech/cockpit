use marqueur_commande::commande;
mod agents;
mod appearance;
mod evenements;
mod ouvrir;
mod taches;
pub mod pont;
mod chemins;
pub mod compte;
mod commande;
mod docker;
mod gitdiff;
mod llm;
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

use docker::orchestrator::Orchestrator;
use std::sync::Arc;
use storage::Database;
use system::metrics::Collector;
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
/// EFFETS qui precedent (ouvrir la base, preparer les terminaux), parce qu'ils dependent
/// de l'hote et pas de l'etat.
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


// --- Tauri Commands: Docker ---

#[derive(serde::Serialize)]
pub struct ProjectWithFolder {
    #[serde(flatten)]
    project: docker::orchestrator::Project,
    folder_id: Option<i64>,
}


/// La logique de `list_projects`, appelable par TOUT hote. Le corps n'a pas bouge : seule
/// la signature change, `&AppState` se lisant comme `State<AppState>` par deref.
#[commande]
pub async fn list_projects(
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
#[commande]
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


/// La logique de `start_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn start_project(state: &AppState, name: String) -> Result<(), String> {
    state.orchestrator.start_project(&name).await
}


/// La logique de `stop_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn stop_project(state: &AppState, name: String) -> Result<(), String> {
    state.orchestrator.stop_project(&name).await
}


/// La logique de `restart_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn restart_project(state: &AppState, name: String) -> Result<(), String> {
    state.orchestrator.restart_project(&name).await
}

// --- Tauri Commands: Conteneurs Docker (vue globale) ---

#[commande]
async fn list_all_containers() -> Result<Vec<docker::containers::DockerContainer>, String> {
    docker::containers::list_all().await
}

#[commande]
async fn container_action(id: String, action: String) -> Result<(), String> {
    docker::containers::container_action(&id, &action).await
}

#[commande]
async fn container_logs(id: String, tail: u32) -> Result<String, String> {
    docker::containers::container_logs(&id, tail).await
}

#[commande]
async fn container_action_bulk(ids: Vec<String>, action: String) -> Result<(), String> {
    docker::containers::container_action_bulk(&ids, &action).await
}

#[commande]
async fn docker_disk_usage() -> Result<Vec<docker::containers::DiskUsage>, String> {
    docker::containers::disk_usage().await
}

#[commande]
async fn list_docker_volumes() -> Result<Vec<docker::containers::DockerVolume>, String> {
    docker::containers::list_volumes().await
}

#[commande]
async fn list_docker_images() -> Result<Vec<docker::containers::DockerImage>, String> {
    docker::containers::list_images().await
}

#[commande]
async fn remove_docker_volume(name: String) -> Result<(), String> {
    docker::containers::remove_volume(&name).await
}

#[commande]
async fn remove_docker_image(id: String) -> Result<(), String> {
    docker::containers::remove_image(&id).await
}

#[commande]
async fn docker_prune(target: String) -> Result<String, String> {
    docker::containers::prune(&target).await
}

// --- Tauri Commands: Todos ---


/// La logique de `get_todos`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_todos(state: &AppState, project: String) -> Result<Vec<storage::Todo>, String> {
    state.db.get_todos(&project)
}


/// La logique de `create_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn create_todo(state: &AppState, project: String, text: String) -> Result<storage::Todo, String> {
    state.db.create_todo(&project, &text)
}


/// La logique de `update_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn update_todo(state: &AppState, id: i64, text: String, done: bool) -> Result<storage::Todo, String> {
    state.db.update_todo(id, &text, done)
}


/// La logique de `set_todo_due`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn set_todo_due(state: &AppState, id: i64, due_date: Option<String>) -> Result<storage::Todo, String> {
    state.db.set_todo_due(id, due_date.as_deref())
}

/// Avancement d'une tache, en pourcentage. 100 la marque finie.
/// La logique de `set_todo_progress`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn set_todo_progress(state: &AppState, id: i64, progress: i32) -> Result<storage::Todo, String> {
    state.db.set_todo_progress(id, progress)
}


/// La logique de `delete_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn delete_todo(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_todo(id)
}


/// La logique de `reorder_todos`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn reorder_todos(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_todos(&ids)
}


/// La logique de `move_todo`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn move_todo(state: &AppState, id: i64, new_project: String) -> Result<(), String> {
    state.db.move_todo(id, &new_project)
}


/// La logique de `get_pending_todos`, appelable par TOUT hote. La commande Tauri
/// ci-dessus n'en est que la facade, et le pont appelle celle-ci : une seule verite.
#[commande]
pub fn get_pending_todos(state: &AppState) -> Result<Vec<storage::Todo>, String> {
    state.db.get_pending_todos()
}

// --- Tauri Commands: Notes ---


/// La logique de `get_note`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_note(state: &AppState, project: String) -> Result<Option<storage::Note>, String> {
    state.db.get_note(&project)
}


/// La logique de `save_note`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn save_note(state: &AppState, project: String, content: String) -> Result<storage::Note, String> {
    state.db.save_note(&project, &content)
}


/// La logique de `get_note_tree`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_note_tree(state: &AppState, project: String) -> Result<storage::NoteTree, String> {
    state.db.get_note_tree(&project)
}


/// La logique de `create_note_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn create_note_folder(state: &AppState, project: String, parent_id: Option<i64>, name: String) -> Result<storage::NoteFolder, String> {
    state.db.create_note_folder(&project, parent_id, &name)
}


/// La logique de `rename_note_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn rename_note_folder(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.db.rename_note_folder(id, &name)
}


/// La logique de `delete_note_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn delete_note_folder(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_note_folder(id)
}


/// La logique de `create_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn create_note_file(state: &AppState, project: String, folder_id: Option<i64>, name: String) -> Result<storage::NoteFile, String> {
    state.db.create_note_file(&project, folder_id, &name)
}


/// La logique de `get_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_note_file(state: &AppState, id: i64) -> Result<storage::NoteFile, String> {
    state.db.get_note_file(id)
}


/// La logique de `save_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn save_note_file(state: &AppState, id: i64, content: String) -> Result<(), String> {
    state.db.save_note_file(id, &content)
}


/// La logique de `rename_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn rename_note_file(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.db.rename_note_file(id, &name)
}


/// La logique de `delete_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn delete_note_file(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_note_file(id)
}


/// La logique de `reorder_note_folders`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn reorder_note_folders(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_note_folders(&ids)
}


/// La logique de `reorder_note_files`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn reorder_note_files(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_note_files(&ids)
}


/// La logique de `move_note_file`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn move_note_file(state: &AppState, id: i64, folder_id: Option<i64>) -> Result<(), String> {
    state.db.move_note_file(id, folder_id)
}

// --- Tauri Commands: URLs ---


/// La logique de `get_urls`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_urls(state: &AppState, project: String) -> Result<Vec<storage::Url>, String> {
    state.db.get_urls(&project)
}

#[commande]
async fn check_urls(urls: Vec<String>) -> Vec<urlhealth::UrlHealth> {
    urlhealth::check_urls(&urls).await
}


/// La logique de `create_url`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn create_url(state: &AppState, project: String, label: String, url: String) -> Result<storage::Url, String> {
    state.db.create_url(&project, &label, &url)
}


/// La logique de `update_url`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn update_url(state: &AppState, id: i64, label: String, url: String) -> Result<storage::Url, String> {
    state.db.update_url(id, &label, &url)
}


/// La logique de `delete_url`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn delete_url(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_url(id)
}

// --- Tauri Commands: Commandes rapides par projet ---


/// La logique de `get_project_commands`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_project_commands(state: &AppState, project: String) -> Result<Vec<storage::ProjectCommand>, String> {
    state.db.get_project_commands(&project)
}


/// La logique de `create_project_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn create_project_command(state: &AppState, project: String, label: String, command: String) -> Result<storage::ProjectCommand, String> {
    state.db.create_project_command(&project, &label, &command)
}


/// La logique de `update_project_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn update_project_command(state: &AppState, id: i64, label: String, command: String) -> Result<storage::ProjectCommand, String> {
    state.db.update_project_command(id, &label, &command)
}


/// La logique de `delete_project_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn delete_project_command(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_project_command(id)
}


/// La logique de `reorder_project_commands`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn reorder_project_commands(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_project_commands(&ids)
}

// --- Tauri Commands: Project Folders ---


/// La logique de `get_project_folders`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_project_folders(state: &AppState) -> Result<Vec<storage::ProjectFolder>, String> {
    state.db.get_project_folders()
}


/// La logique de `create_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn create_project_folder(state: &AppState, name: String, parent_id: Option<i64>) -> Result<storage::ProjectFolder, String> {
    state.db.create_project_folder(&name, parent_id)
}


/// La logique de `rename_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn rename_project_folder(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.db.rename_project_folder(id, &name)
}


/// La logique de `delete_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn delete_project_folder(state: &AppState, id: i64) -> Result<(), String> {
    state.db.delete_project_folder(id)
}


/// La logique de `reorder_project_folders`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn reorder_project_folders(state: &AppState, ids: Vec<i64>) -> Result<(), String> {
    state.db.reorder_project_folders(&ids)
}

/// Deplace un dossier sous un autre (`parent_id` a None = racine). Refuse les boucles.
/// La logique de `move_project_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn move_project_folder(state: &AppState, id: i64, parent_id: Option<i64>) -> Result<(), String> {
    state.db.move_project_folder(id, parent_id)
}


/// La logique de `move_project_to_folder`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn move_project_to_folder(state: &AppState, project_name: String, folder_id: Option<i64>) -> Result<(), String> {
    state.db.move_project_to_folder(&project_name, folder_id)
}

// --- Tauri Commands: Scanner ---

#[commande]
async fn scan_dir(path: String) -> Result<scanner::ScanResult, String> {
    scanner::scan(&path)
}

#[commande]
async fn scan_subdirs(path: String) -> Result<Vec<scanner::ScanResult>, String> {
    scanner::scan_subdirs(&path)
}

// --- Tauri Commands: Settings (DB projects) ---


/// La logique de `get_db_projects`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_db_projects(state: &AppState) -> Result<Vec<storage::Project>, String> {
    state.db.get_projects()
}


/// La logique de `add_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn add_project(state: &AppState, name: String, path: String, compose_file: String, description: String, depends_on: Vec<String>) -> Result<storage::Project, String> {
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


/// La logique de `update_db_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn update_db_project(state: &AppState, id: i64, name: String, path: String, compose_file: String, description: String, depends_on: Vec<String>) -> Result<storage::Project, String> {
    state.db.update_project(id, &name, &path, &compose_file, &description, &depends_on)
}


/// La logique de `delete_db_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn delete_db_project(state: &AppState, id: i64) -> Result<(), String> {
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


/// La logique de `reorder_projects`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn reorder_projects(state: &AppState, names: Vec<String>) -> Result<(), String> {
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


/// La logique de `docker_compose_detecte`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn docker_compose_detecte(state: &AppState, name: String, rafraichir: bool) -> Result<ComposeDetecte, String> {
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


/// La logique de `get_project_settings`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn get_project_settings(state: &AppState, name: String) -> Result<storage::Project, String> {
    let db_name = resolve_db_project_name(&state, &name).await;
    state.db.get_project_by_name(&db_name)
}


/// La logique de `update_project_settings`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn update_project_settings(state: &AppState, name: String, path: String, compose_file: String, description: String, depends_on: Vec<String>) -> Result<storage::Project, String> {
    let db_name = resolve_db_project_name(&state, &name).await;
    let proj = state.db.update_project_by_name(&db_name, &path, &compose_file, &description, &depends_on)?;
    // L'orchestrateur est indexe par le nom AFFICHE, lui
    state.orchestrator.update_project(&name, &path, &compose_file, &description, depends_on).await;
    Ok(proj)
}


/// La logique de `rename_project`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn rename_project(state: &AppState, old_name: String, new_name: String) -> Result<(), String> {
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


/// La logique de `get_system_metrics`, appelable par TOUT hote.
#[commande]
pub async fn get_system_metrics(
    state: &AppState,
) -> Result<system::metrics::SystemMetrics, String> {
    let mut collector = state.collector.lock().await;
    Ok(collector.collect())
}


/// La logique de `kill_process`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn kill_process(state: &AppState, pid: u32) -> Result<(), String> {
    let collector = state.collector.lock().await;
    system::process::kill_process_with_sys(collector.system(), pid)
}

// --- Tauri Commands: Apparence (image de fond) ---

#[commande]
async fn set_wallpaper(data_url: String) -> Result<(), String> {
    set_wallpaper_pour_hote(data_url)
}

/// La logique de `set_wallpaper`. L'`AppHandle` n'y servait qu'a trouver le dossier de donnees,
/// que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
fn set_wallpaper_pour_hote(data_url: String) -> Result<(), String> {
    let dossier = crate::chemins::dossier_donnees().ok_or("dossier de donnees inconnu")?;
    appearance::set_wallpaper(dossier, &data_url)
}

#[commande]
async fn get_wallpaper() -> Result<Option<String>, String> {
    get_wallpaper_pour_hote()
}

/// La logique de `get_wallpaper`. L'`AppHandle` n'y servait qu'a trouver le dossier de donnees,
/// que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
fn get_wallpaper_pour_hote() -> Result<Option<String>, String> {
    let dossier = crate::chemins::dossier_donnees().ok_or("dossier de donnees inconnu")?;
    appearance::get_wallpaper(dossier)
}

#[commande]
async fn clear_wallpaper() -> Result<(), String> {
    clear_wallpaper_pour_hote()
}

/// La logique de `clear_wallpaper`. L'`AppHandle` n'y servait qu'a trouver le dossier de donnees,
/// que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
fn clear_wallpaper_pour_hote() -> Result<(), String> {
    let dossier = crate::chemins::dossier_donnees().ok_or("dossier de donnees inconnu")?;
    appearance::clear_wallpaper(dossier)
}

#[commande]
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
// --- Tauri Command: Import DB ---


/// La logique de `import_database`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn import_database(state: &AppState, path: String) -> Result<String, String> {
    storage::import::import_from(&state.db, &path)
}


/// La logique de `get_db_path`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_db_path(state: &AppState) -> String {
    state.db_path.clone()
}

// --- Tauri Commands: Enregistrement de reunions ---


/// La logique de `start_recording`. L'`AppHandle` n'y servait qu'a emettre l'etat de
/// l'enregistrement : l'emetteur de l'etat le fait, sans connaitre l'hote.
#[commande]
async fn start_recording(
    state: &AppState,
    project: String,
) -> Result<recorder::RecordingStatus, String> {
    recorder::start(state.emetteur.clone(), state.db.clone(), &state.recorder, project).await
}


/// La logique de `stop_recording`.
#[commande]
async fn stop_recording(state: &AppState) -> Result<(), String> {
    recorder::stop(state.emetteur.clone(), state.db.clone(), &state.recorder).await
}


/// La logique de `get_active_recording`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_active_recording(state: &AppState) -> Option<recorder::RecordingStatus> {
    recorder::active_status(&state.recorder)
}


/// La logique de `get_failed_recordings`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn get_failed_recordings(state: &AppState, project: String) -> Result<Vec<storage::Recording>, String> {
    state.db.get_failed_recordings(&project)
}


/// La logique de `retry_recording`.
#[commande]
fn retry_recording(state: &AppState, id: i64) -> Result<(), String> {
    recorder::retry(state.emetteur.clone(), state.db.clone(), id)
}


/// La logique de `delete_recording`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn delete_recording(state: &AppState, id: i64) -> Result<(), String> {
    recorder::delete(&state.db, id)
}

// --- Tauri Commands: App settings (cle API, prompt de resume) ---


/// La logique de `get_app_settings`, appelable par TOUT hote.
#[commande]
pub fn get_app_settings(
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


/// La logique de `set_app_setting`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn set_app_setting(state: &AppState, key: String, value: String) -> Result<(), String> {
    state.db.set_setting(&key, &value)
}


/// La logique de `get_project_summary_prompt`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn get_project_summary_prompt(state: &AppState, project: String) -> Result<Option<String>, String> {
    let db_name = resolve_db_project_name(&state, &project).await;
    state.db.get_project_summary_prompt(&db_name)
}


/// La logique de `set_project_summary_prompt`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn set_project_summary_prompt(state: &AppState, project: String, prompt: Option<String>) -> Result<(), String> {
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


/// La logique de `create_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn create_terminal(state: &AppState, project: String, cwd: String, cols: u16, rows: u16, init_command: Option<String>) -> Result<i64, String> {
    let demande = terminal::Creation {
        projet: project,
        dossier: cwd,
        taille: terminal::Taille { colonnes: cols, lignes: rows },
        commande_initiale: init_command,
    };
    state.terminals.creer(&state.db, demande)
}


/// La logique de `write_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn write_terminal(state: &AppState, id: i64, data: String) -> Result<(), String> {
    state.terminals.ecrire(id, &data)
}


/// La logique de `resize_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn resize_terminal(state: &AppState, id: i64, cols: u16, rows: u16) -> Result<(), String> {
    state
        .terminals
        .redimensionner(&state.db, id, terminal::Taille { colonnes: cols, lignes: rows })
}


/// La logique de `close_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn close_terminal(state: &AppState, id: i64) -> Result<(), String> {
    state.terminals.fermer(&state.db, id)
}

/// Photographie les terminaux ouverts pour qu'ils reviennent « comme on les a quittes ».
///
/// Appelee par l'interface quand on quitte la vue des terminaux — pas sur un minuteur : le
/// cout se paie par terminal, et l'implementation refuse de recommencer avant une minute. La
/// fenetre qui se ferme declenche la meme chose, mais sans borne (voir `fenetre.rs`).
/// La logique de `save_terminal_screens`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn save_terminal_screens(state: &AppState) -> Result<(), String> {
    state.terminals.enregistrer_les_ecrans(&state.db, false);
    Ok(())
}


/// La logique de `attach_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn attach_terminal(state: &AppState, id: i64, cols: u16, rows: u16) -> Result<(), String> {
    state
        .terminals
        .attacher(&state.db, id, terminal::Taille { colonnes: cols, lignes: rows })
}


/// La logique de `rename_terminal`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn rename_terminal(state: &AppState, id: i64, name: String) -> Result<(), String> {
    state.terminals.renommer(&state.db, id, &name)
}


/// La logique de `list_terminals`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn list_terminals(state: &AppState, project: String) -> Result<Vec<terminal::TerminalInfo>, String> {
    Ok(state.terminals.lister(&state.db, Some(&project)))
}


/// La logique de `list_all_terminals`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn list_all_terminals(state: &AppState) -> Result<Vec<terminal::TerminalInfo>, String> {
    Ok(state.terminals.lister(&state.db, None))
}

/// Presse-papier systeme. Instance arboard gardee en vie : sous X11 le contenu
/// du presse-papier disparait quand son proprietaire (la connexion) est droppe.
static CLIPBOARD: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);

#[commande]
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

#[commande]
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
/// La logique de `llm_catalogue`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_catalogue(state: &AppState) -> Vec<llm::Capacites> {
    llm::catalogue_pour_le_frontend(&state.db)
}


/// La logique de `llm_choisir`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_choisir(state: &AppState, id: String) -> Result<(), String> {
    llm::choisir(&state.db, &id)
}


/// La logique de `llm_poser_cle`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_poser_cle(state: &AppState, id: String, cle: String) -> Result<(), String> {
    llm::poser_cle_api(&state.db, &id, &cle)
}

/// Les conversations passees du projet, chez le fournisseur choisi.
/// La logique de `llm_conversations`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_conversations(state: &AppState, project_path: String) -> Result<Vec<llm::Conversation>, String> {
    llm::conversations::lister(&state.db, llm::prefere(&state.db), &project_path)
}


/// La logique de `llm_renommer_conversation`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_renommer_conversation(state: &AppState, conversation_id: String, nom: String) -> Result<(), String> {
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


/// La logique de `llm_commandes`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_commandes(state: &AppState, conversation_id: Option<String>) -> Result<CommandesAgent, String> {
    let fournisseur = llm::prefere(&state.db);
    let lecteur = fournisseur
        .conversations()
        .ok_or_else(|| format!("{} n'a pas de conversations a reprendre", fournisseur.nom()))?;
    Ok(CommandesAgent {
        neuve: lecteur.commande_neuve(),
        reprise: conversation_id.map(|id| lecteur.commande_de_reprise(&id)),
    })
}


/// La logique de `record_command`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn record_command(state: &AppState, project: String, command: String) -> Result<(), String> {
    terminal::history::record(&state.db, &project, &command)
}


/// La logique de `terminal_search`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn terminal_search(state: &AppState, id: i64, action: String, query: String) -> Result<terminal::ResultatRecherche, String> {
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


/// La logique de `llm_reunions`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_reunions(state: &AppState) -> AffectationsReunion {
    AffectationsReunion {
        transcription: llm::pour(&state.db, |f| f.transcription()).map(|(f, _)| f.nom().to_string()),
        redaction: llm::pour(&state.db, |f| f.texte()).map(|(f, _)| f.nom().to_string()),
    }
}

// --- Tauri Commands: abonnement d'un fournisseur ---

/// L'etat de connexion du fournisseur donne, ou du fournisseur choisi.
/// La logique de `llm_abonnement`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_abonnement(state: &AppState, id: Option<String>) -> Result<llm::EtatAbonnement, String> {
    let fournisseur = match id {
        Some(id) => llm::par_id(&id).ok_or_else(|| format!("fournisseur inconnu : {id}"))?,
        None => llm::prefere(&state.db),
    };
    Ok(llm::abonnement::etat(fournisseur))
}


/// La logique de `llm_connexion_demarrer`. L'`AppHandle` n'y servait qu'a emettre la
/// sortie de la connexion guidee : l'emetteur de l'etat le fait, sans connaitre l'hote.
#[commande]
fn llm_connexion_demarrer(
    state: &AppState,
    id: Option<String>,
) -> Result<(), String> {
    let fournisseur = match id {
        Some(id) => llm::par_id(&id).ok_or_else(|| format!("fournisseur inconnu : {id}"))?,
        None => llm::prefere(&state.db),
    };
    state.connexion_llm.demarrer(state.emetteur.clone(), fournisseur)
}


/// La logique de `llm_connexion_entrer`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_connexion_entrer(state: &AppState, data: String) -> Result<(), String> {
    state.connexion_llm.entrer(&data)
}


/// La logique de `llm_connexion_annuler`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn llm_connexion_annuler(state: &AppState)  {
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

#[commande]
fn open_url(url: String) -> Result<(), String> {
    if !schema_ouvrable(&url) {
        return Err(format!("adresse non ouvrable : {url}"));
    }
    ouvrir::adresse(&url)
}


/// La logique de `report_error`, appelable par tout hote.
///
/// **L'`AppHandle` A DISPARU DE LA SIGNATURE.** Il n'y servait qu'a trouver le dossier de
/// donnees, que `chemins::dossier_donnees()` connait deja, pose par l'hote au demarrage.
/// C'est le meme decouplage que celui du journal des terminaux : garder le handle pour ca
/// rendait la commande inutilisable ailleurs sans aucune raison.
#[commande]
async fn report_error(
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

#[commande]
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

/// Ecrit une ligne dans un fichier de mise au point, hors du journal de l'application.
#[commande]
async fn debug_log(line: String) {
    use std::io::Write;
    // `temp_dir()` et pas `/tmp` : sous Windows le dossier temporaire est dans le profil de
    // l'utilisateur, et un chemin absolu `/tmp` y designerait la racine du lecteur courant.
    let chemin = std::env::temp_dir().join("cockpit-debug.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(chemin) {
        let _ = writeln!(f, "{}", line);
    }
}


/// La logique de `search_command_history`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
fn search_command_history(state: &AppState, query: String, limit: Option<usize>) -> Vec<terminal::history::HistoryEntry> {
    terminal::history::search(&state.db, &query, limit.unwrap_or(50))
}

// --- Tauri Commands: Explorateur de fichiers ---

#[commande]
async fn list_project_dir(project_path: String, rel_path: String) -> Result<Vec<workspace::DirEntry>, String> {
    workspace::list_dir(&project_path, &rel_path)
}

#[commande]
async fn read_project_file(project_path: String, rel_path: String) -> Result<workspace::FileContent, String> {
    workspace::read_project_file(&project_path, &rel_path)
}

/// Etat disque du fichier affiche : sert au suivi des modifications exterieures
/// (relire 2 Mo toutes les deux secondes serait absurde, un stat ne coute rien).
#[commande]
async fn stat_project_file(
    project_path: String,
    rel_path: String,
) -> Result<Option<workspace::FileStat>, String> {
    workspace::stat_project_file(&project_path, &rel_path)
}

#[commande]
async fn write_project_file(project_path: String, rel_path: String, content: String) -> Result<(), String> {
    workspace::write_project_file(&project_path, &rel_path, &content)
}

#[commande]
async fn read_project_image(project_path: String, rel_path: String) -> Result<String, String> {
    workspace::read_project_image(&project_path, &rel_path)
}


/// La logique de `backup_database`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn backup_database(state: &AppState, dest: String) -> Result<(), String> {
    state.db.backup_to(&dest)
}

#[commande]
async fn create_project_file(project_path: String, rel_dir: String, name: String) -> Result<String, String> {
    workspace::create_project_file(&project_path, &rel_dir, &name)
}

#[commande]
async fn create_project_dir(project_path: String, rel_dir: String, name: String) -> Result<String, String> {
    workspace::create_project_dir(&project_path, &rel_dir, &name)
}

#[commande]
async fn rename_project_entry(project_path: String, rel_path: String, new_name: String) -> Result<String, String> {
    workspace::rename_project_entry(&project_path, &rel_path, &new_name)
}

#[commande]
async fn trash_project_entry(project_path: String, rel_path: String) -> Result<(), String> {
    workspace::trash_project_entry(&project_path, &rel_path)
}

// async : la recherche parcourt tout le projet, elle ne doit pas bloquer le thread principal
#[commande]
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
/// La logique de `goto_definition`, appelable par tout hote. La commande ci-dessus
/// n'en est plus que la facade : le corps, lui, n'a pas bouge.
#[commande]
async fn goto_definition(state: &AppState, project_path: String, lang: String, rel_path: String, content: String, line: u32, character: u32, symbol: String) -> Result<GotoDefinitionResult, String> {
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

#[commande]
async fn git_status(project_path: String) -> Result<gitdiff::GitStatus, String> {
    gitdiff::git_status(&project_path).await
}

#[commande]
async fn git_diff_file(
    project_path: String,
    path: String,
    untracked: bool,
) -> Result<gitdiff::FileDiff, String> {
    gitdiff::git_diff_file(&project_path, &path, untracked).await
}

#[commande]
async fn git_stage(project_path: String, path: String) -> Result<(), String> {
    gitdiff::git_stage(&project_path, &path).await
}

#[commande]
async fn git_unstage(project_path: String, path: String) -> Result<(), String> {
    gitdiff::git_unstage(&project_path, &path).await
}

#[commande]
async fn git_stage_all(project_path: String) -> Result<(), String> {
    gitdiff::git_stage_all(&project_path).await
}

#[commande]
async fn git_unstage_all(project_path: String) -> Result<(), String> {
    gitdiff::git_unstage_all(&project_path).await
}

#[commande]
async fn git_commit(project_path: String, message: String) -> Result<(), String> {
    gitdiff::git_commit(&project_path, &message).await
}

#[commande]
async fn git_push(project_path: String, set_upstream: bool) -> Result<String, String> {
    gitdiff::git_push(&project_path, set_upstream).await
}

#[commande]
async fn git_pull(project_path: String) -> Result<String, String> {
    gitdiff::git_pull(&project_path).await
}

#[commande]
async fn git_log(project_path: String, limit: u32) -> Result<Vec<gitdiff::CommitInfo>, String> {
    gitdiff::git_log(&project_path, limit).await
}

#[commande]
async fn git_commit_diff(project_path: String, hash: String) -> Result<Vec<gitdiff::FileDiff>, String> {
    gitdiff::git_commit_diff(&project_path, &hash).await
}

#[commande]
async fn git_branches(project_path: String) -> Result<Vec<gitdiff::BranchInfo>, String> {
    gitdiff::git_branches(&project_path).await
}

/// Les dossiers de travail du depot, le principal en premier.
#[commande]
async fn git_worktrees(project_path: String) -> Result<Vec<gitdiff::worktree::Worktree>, String> {
    gitdiff::worktree::lister(&project_path).await
}

/// Ajoute un dossier de travail sur `branche`, en la creant si `creer`. Rend son chemin.
#[commande]
async fn git_worktree_add(
    project_path: String,
    branche: String,
    creer: bool,
) -> Result<String, String> {
    gitdiff::worktree::ajouter(&project_path, &branche, creer).await
}

/// Retire un dossier de travail. `force` abandonne les modifications non validees.
#[commande]
async fn git_worktree_remove(
    project_path: String,
    chemin: String,
    force: bool,
) -> Result<(), String> {
    gitdiff::worktree::retirer(&project_path, &chemin, force).await
}

#[commande]
async fn git_checkout_branch(project_path: String, name: String) -> Result<(), String> {
    gitdiff::git_checkout_branch(&project_path, &name).await
}

#[commande]
async fn git_create_branch(project_path: String, name: String) -> Result<(), String> {
    gitdiff::git_create_branch(&project_path, &name).await
}

#[commande]
async fn git_delete_branch(project_path: String, name: String, force: bool) -> Result<(), String> {
    gitdiff::git_delete_branch(&project_path, &name, force).await
}

// --- Tauri Commands: Agents marketplace (multi-marketplace) ---

#[commande]
fn get_marketplace_path() -> Result<String, String> {
    Ok(agents::ccm_marketplace_path()?.to_string_lossy().to_string())
}

#[commande]
fn list_marketplaces() -> Result<Vec<agents::MarketplaceLocation>, String> {
    agents::list_marketplaces()
}

#[commande]
fn list_plugins(marketplace_id: String) -> Result<Vec<agents::PluginInfo>, String> {
    agents::list_plugins_in(&marketplace_id)
}

#[commande]
fn list_agents(marketplace_id: String, plugin: String) -> Result<Vec<agents::AgentInfo>, String> {
    agents::list_agents_in(&marketplace_id, &plugin)
}

#[commande]
fn read_agent(marketplace_id: String, plugin: String, name: String) -> Result<String, String> {
    agents::read_agent(&marketplace_id, &plugin, &name)
}

#[commande]
fn save_agent(
    marketplace_id: String,
    plugin: String,
    name: String,
    content: String,
) -> Result<(), String> {
    agents::save_agent(&marketplace_id, &plugin, &name, &content)
}

#[commande]
fn delete_agent(marketplace_id: String, plugin: String, name: String) -> Result<(), String> {
    agents::delete_agent(&marketplace_id, &plugin, &name)
}

#[commande]
fn rename_agent(
    marketplace_id: String,
    plugin: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    agents::rename_agent(&marketplace_id, &plugin, &old_name, &new_name)
}

#[commande]
fn create_plugin(name: String, description: String) -> Result<(), String> {
    agents::create_plugin(&name, &description)
}

#[commande]
fn delete_plugin(marketplace_id: String, name: String) -> Result<(), String> {
    agents::delete_plugin(&marketplace_id, &name)
}

#[commande]
fn rename_plugin(
    marketplace_id: String,
    old_name: String,
    new_name: String,
) -> Result<(), String> {
    agents::rename_plugin(&marketplace_id, &old_name, &new_name)
}

#[commande]
fn get_project_plugins(project_path: String) -> Result<Vec<String>, String> {
    agents::get_project_plugins(&project_path)
}

#[commande]
fn set_project_plugins(project_path: String, plugins: Vec<String>) -> Result<(), String> {
    agents::set_project_plugins(&project_path, plugins)
}

#[commande]
fn get_orchestrator_config() -> Result<agents::OrchestratorConfig, String> {
    agents::get_orchestrator_config()
}

#[commande]
fn set_teams_enabled(enabled: bool) -> Result<(), String> {
    agents::set_teams_enabled(enabled)
}

#[commande]
fn set_teammate_mode(mode: String) -> Result<(), String> {
    agents::set_teammate_mode(&mode)
}

#[commande]
fn toggle_plugin_enabled(plugin_key: String, enabled: bool) -> Result<(), String> {
    agents::toggle_plugin_enabled(&plugin_key, enabled)
}

// --- App Setup ---

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
