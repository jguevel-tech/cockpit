//! Le dispatch des commandes du pont. **GENERE — ne pas editer a la main** :
//! `scripts/pont-dispatch.py` le reconstruit depuis les commandes de `lib.rs`, et une
//! retouche ici serait perdue au prochain passage.
//!
//! **LES ARGUMENTS ARRIVENT EN camelCase.** C'est la macro de Tauri qui les convertit
//! aujourd'hui ; sans cette conversion ici, toute commande a arguments echouerait alors
//! que la meme marche sous Tauri. Le nom en snake_case est accepte aussi, comme le fait
//! Tauri, pour qu'un appel ecrit a la main ne soit pas refuse sans raison lisible.

use crate::AppState;
use serde_json::Value;

/// Un argument, cherche sous ses deux noms. Absent, il vaut `null` : c'est `serde` qui
/// tranche ensuite, et un `Option<T>` l'accepte la ou un `T` le refuse avec un message
/// qui nomme le champ.
fn prendre(a: &Value, camel: &str, snake: &str) -> Value {
    a.get(camel).or_else(|| a.get(snake)).cloned().unwrap_or(Value::Null)
}

/// Donne au bloc de chaque branche le type que `?` reclame. Sans elle, le compilateur voit
/// une fonction qui rend `Option` et refuse tout `?` sur un `Result` : c'est ce qui a fait
/// echouer la premiere version generee.
fn typer<F: std::future::Future<Output = Result<Value, String>>>(f: F) -> F {
    f
}

/// Repond si la commande est connue, `None` sinon — le pont ajoute ses propres branches
/// et NOMME ce qui reste inconnu.
pub async fn appeler(
    etat: &AppState,
    commande: &str,
    a: &Value,
) -> Option<Result<Value, String>> {
    let valeur = |v: Result<Value, serde_json::Error>| v.map_err(|e| e.to_string());
    Some(match commande {
        "list_projects" => typer(async {
            valeur(serde_json::to_value(crate::list_projects(etat, 
            ).await?))
        })
        .await,
        "langue_imposee" => typer(async {
            valeur(serde_json::to_value(crate::langue_imposee(
            )))
        })
        .await,
        "start_project" => typer(async {
            valeur(serde_json::to_value(crate::start_project(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "stop_project" => typer(async {
            valeur(serde_json::to_value(crate::stop_project(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "restart_project" => typer(async {
            valeur(serde_json::to_value(crate::restart_project(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "list_all_containers" => typer(async {
            valeur(serde_json::to_value(crate::list_all_containers(
            ).await?))
        })
        .await,
        "container_action" => typer(async {
            valeur(serde_json::to_value(crate::container_action(
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "action", "action"))
                    .map_err(|e| format!("argument action : {e}"))?,
            ).await?))
        })
        .await,
        "container_logs" => typer(async {
            valeur(serde_json::to_value(crate::container_logs(
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "tail", "tail"))
                    .map_err(|e| format!("argument tail : {e}"))?,
            ).await?))
        })
        .await,
        "container_action_bulk" => typer(async {
            valeur(serde_json::to_value(crate::container_action_bulk(
                serde_json::from_value(prendre(a, "ids", "ids"))
                    .map_err(|e| format!("argument ids : {e}"))?,
                serde_json::from_value(prendre(a, "action", "action"))
                    .map_err(|e| format!("argument action : {e}"))?,
            ).await?))
        })
        .await,
        "docker_disk_usage" => typer(async {
            valeur(serde_json::to_value(crate::docker_disk_usage(
            ).await?))
        })
        .await,
        "list_docker_volumes" => typer(async {
            valeur(serde_json::to_value(crate::list_docker_volumes(
            ).await?))
        })
        .await,
        "list_docker_images" => typer(async {
            valeur(serde_json::to_value(crate::list_docker_images(
            ).await?))
        })
        .await,
        "remove_docker_volume" => typer(async {
            valeur(serde_json::to_value(crate::remove_docker_volume(
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "remove_docker_image" => typer(async {
            valeur(serde_json::to_value(crate::remove_docker_image(
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            ).await?))
        })
        .await,
        "docker_prune" => typer(async {
            valeur(serde_json::to_value(crate::docker_prune(
                serde_json::from_value(prendre(a, "target", "target"))
                    .map_err(|e| format!("argument target : {e}"))?,
            ).await?))
        })
        .await,
        "get_todos" => typer(async {
            valeur(serde_json::to_value(crate::get_todos(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            )?))
        })
        .await,
        "create_todo" => typer(async {
            valeur(serde_json::to_value(crate::create_todo(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "text", "text"))
                    .map_err(|e| format!("argument text : {e}"))?,
            )?))
        })
        .await,
        "update_todo" => typer(async {
            valeur(serde_json::to_value(crate::update_todo(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "text", "text"))
                    .map_err(|e| format!("argument text : {e}"))?,
                serde_json::from_value(prendre(a, "done", "done"))
                    .map_err(|e| format!("argument done : {e}"))?,
            )?))
        })
        .await,
        "set_todo_due" => typer(async {
            valeur(serde_json::to_value(crate::set_todo_due(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "dueDate", "due_date"))
                    .map_err(|e| format!("argument dueDate : {e}"))?,
            )?))
        })
        .await,
        "set_todo_progress" => typer(async {
            valeur(serde_json::to_value(crate::set_todo_progress(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "progress", "progress"))
                    .map_err(|e| format!("argument progress : {e}"))?,
            )?))
        })
        .await,
        "delete_todo" => typer(async {
            valeur(serde_json::to_value(crate::delete_todo(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "reorder_terminals" => typer(async {
            valeur(serde_json::to_value(crate::reorder_terminals(etat,
                serde_json::from_value(prendre(a, "ids", "ids"))
                    .map_err(|e| format!("argument ids : {e}"))?,
            )?))
        })
        .await,
        "reorder_todos" => typer(async {
            valeur(serde_json::to_value(crate::reorder_todos(etat,
                serde_json::from_value(prendre(a, "ids", "ids"))
                    .map_err(|e| format!("argument ids : {e}"))?,
            )?))
        })
        .await,
        "move_todo" => typer(async {
            valeur(serde_json::to_value(crate::move_todo(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "newProject", "new_project"))
                    .map_err(|e| format!("argument newProject : {e}"))?,
            )?))
        })
        .await,
        "get_pending_todos" => typer(async {
            valeur(serde_json::to_value(crate::get_pending_todos(etat, 
            )?))
        })
        .await,
        "get_note" => typer(async {
            valeur(serde_json::to_value(crate::get_note(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            )?))
        })
        .await,
        "save_note" => typer(async {
            valeur(serde_json::to_value(crate::save_note(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "content", "content"))
                    .map_err(|e| format!("argument content : {e}"))?,
            )?))
        })
        .await,
        "get_note_tree" => typer(async {
            valeur(serde_json::to_value(crate::get_note_tree(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            )?))
        })
        .await,
        "create_note_folder" => typer(async {
            valeur(serde_json::to_value(crate::create_note_folder(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "parentId", "parent_id"))
                    .map_err(|e| format!("argument parentId : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "rename_note_folder" => typer(async {
            valeur(serde_json::to_value(crate::rename_note_folder(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "delete_note_folder" => typer(async {
            valeur(serde_json::to_value(crate::delete_note_folder(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "create_note_file" => typer(async {
            valeur(serde_json::to_value(crate::create_note_file(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "folderId", "folder_id"))
                    .map_err(|e| format!("argument folderId : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "get_note_file" => typer(async {
            valeur(serde_json::to_value(crate::get_note_file(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "save_note_file" => typer(async {
            valeur(serde_json::to_value(crate::save_note_file(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "content", "content"))
                    .map_err(|e| format!("argument content : {e}"))?,
            )?))
        })
        .await,
        "rename_note_file" => typer(async {
            valeur(serde_json::to_value(crate::rename_note_file(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "delete_note_file" => typer(async {
            valeur(serde_json::to_value(crate::delete_note_file(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "reorder_note_folders" => typer(async {
            valeur(serde_json::to_value(crate::reorder_note_folders(etat,
                serde_json::from_value(prendre(a, "ids", "ids"))
                    .map_err(|e| format!("argument ids : {e}"))?,
            )?))
        })
        .await,
        "reorder_note_files" => typer(async {
            valeur(serde_json::to_value(crate::reorder_note_files(etat,
                serde_json::from_value(prendre(a, "ids", "ids"))
                    .map_err(|e| format!("argument ids : {e}"))?,
            )?))
        })
        .await,
        "move_note_file" => typer(async {
            valeur(serde_json::to_value(crate::move_note_file(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "folderId", "folder_id"))
                    .map_err(|e| format!("argument folderId : {e}"))?,
            )?))
        })
        .await,
        "get_urls" => typer(async {
            valeur(serde_json::to_value(crate::get_urls(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            )?))
        })
        .await,
        "check_urls" => typer(async {
            valeur(serde_json::to_value(crate::check_urls(
                serde_json::from_value(prendre(a, "urls", "urls"))
                    .map_err(|e| format!("argument urls : {e}"))?,
            ).await))
        })
        .await,
        "create_url" => typer(async {
            valeur(serde_json::to_value(crate::create_url(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "label", "label"))
                    .map_err(|e| format!("argument label : {e}"))?,
                serde_json::from_value(prendre(a, "url", "url"))
                    .map_err(|e| format!("argument url : {e}"))?,
            )?))
        })
        .await,
        "update_url" => typer(async {
            valeur(serde_json::to_value(crate::update_url(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "label", "label"))
                    .map_err(|e| format!("argument label : {e}"))?,
                serde_json::from_value(prendre(a, "url", "url"))
                    .map_err(|e| format!("argument url : {e}"))?,
            )?))
        })
        .await,
        "delete_url" => typer(async {
            valeur(serde_json::to_value(crate::delete_url(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "get_project_commands" => typer(async {
            valeur(serde_json::to_value(crate::get_project_commands(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            )?))
        })
        .await,
        "create_project_command" => typer(async {
            valeur(serde_json::to_value(crate::create_project_command(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "label", "label"))
                    .map_err(|e| format!("argument label : {e}"))?,
                serde_json::from_value(prendre(a, "command", "command"))
                    .map_err(|e| format!("argument command : {e}"))?,
            )?))
        })
        .await,
        "update_project_command" => typer(async {
            valeur(serde_json::to_value(crate::update_project_command(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "label", "label"))
                    .map_err(|e| format!("argument label : {e}"))?,
                serde_json::from_value(prendre(a, "command", "command"))
                    .map_err(|e| format!("argument command : {e}"))?,
            )?))
        })
        .await,
        "delete_project_command" => typer(async {
            valeur(serde_json::to_value(crate::delete_project_command(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "reorder_project_commands" => typer(async {
            valeur(serde_json::to_value(crate::reorder_project_commands(etat,
                serde_json::from_value(prendre(a, "ids", "ids"))
                    .map_err(|e| format!("argument ids : {e}"))?,
            )?))
        })
        .await,
        "get_project_folders" => typer(async {
            valeur(serde_json::to_value(crate::get_project_folders(etat, 
            )?))
        })
        .await,
        "create_project_folder" => typer(async {
            valeur(serde_json::to_value(crate::create_project_folder(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "parentId", "parent_id"))
                    .map_err(|e| format!("argument parentId : {e}"))?,
            )?))
        })
        .await,
        "rename_project_folder" => typer(async {
            valeur(serde_json::to_value(crate::rename_project_folder(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "delete_project_folder" => typer(async {
            valeur(serde_json::to_value(crate::delete_project_folder(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "reorder_project_folders" => typer(async {
            valeur(serde_json::to_value(crate::reorder_project_folders(etat,
                serde_json::from_value(prendre(a, "ids", "ids"))
                    .map_err(|e| format!("argument ids : {e}"))?,
            )?))
        })
        .await,
        "move_project_folder" => typer(async {
            valeur(serde_json::to_value(crate::move_project_folder(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "parentId", "parent_id"))
                    .map_err(|e| format!("argument parentId : {e}"))?,
            )?))
        })
        .await,
        "move_project_to_folder" => typer(async {
            valeur(serde_json::to_value(crate::move_project_to_folder(etat,
                serde_json::from_value(prendre(a, "projectName", "project_name"))
                    .map_err(|e| format!("argument projectName : {e}"))?,
                serde_json::from_value(prendre(a, "folderId", "folder_id"))
                    .map_err(|e| format!("argument folderId : {e}"))?,
            )?))
        })
        .await,
        "scan_dir" => typer(async {
            valeur(serde_json::to_value(crate::scan_dir(
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
            ).await?))
        })
        .await,
        "scan_subdirs" => typer(async {
            valeur(serde_json::to_value(crate::scan_subdirs(
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
            ).await?))
        })
        .await,
        "get_db_projects" => typer(async {
            valeur(serde_json::to_value(crate::get_db_projects(etat, 
            )?))
        })
        .await,
        "add_project" => typer(async {
            valeur(serde_json::to_value(crate::add_project(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
                serde_json::from_value(prendre(a, "composeFile", "compose_file"))
                    .map_err(|e| format!("argument composeFile : {e}"))?,
                serde_json::from_value(prendre(a, "description", "description"))
                    .map_err(|e| format!("argument description : {e}"))?,
                serde_json::from_value(prendre(a, "dependsOn", "depends_on"))
                    .map_err(|e| format!("argument dependsOn : {e}"))?,
            ).await?))
        })
        .await,
        "update_db_project" => typer(async {
            valeur(serde_json::to_value(crate::update_db_project(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
                serde_json::from_value(prendre(a, "composeFile", "compose_file"))
                    .map_err(|e| format!("argument composeFile : {e}"))?,
                serde_json::from_value(prendre(a, "description", "description"))
                    .map_err(|e| format!("argument description : {e}"))?,
                serde_json::from_value(prendre(a, "dependsOn", "depends_on"))
                    .map_err(|e| format!("argument dependsOn : {e}"))?,
            )?))
        })
        .await,
        "delete_db_project" => typer(async {
            valeur(serde_json::to_value(crate::delete_db_project(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            ).await?))
        })
        .await,
        "reorder_projects" => typer(async {
            valeur(serde_json::to_value(crate::reorder_projects(etat,
                serde_json::from_value(prendre(a, "names", "names"))
                    .map_err(|e| format!("argument names : {e}"))?,
            )?))
        })
        .await,
        "docker_compose_detecte" => typer(async {
            valeur(serde_json::to_value(crate::docker_compose_detecte(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "rafraichir", "rafraichir"))
                    .map_err(|e| format!("argument rafraichir : {e}"))?,
            ).await?))
        })
        .await,
        "get_project_settings" => typer(async {
            valeur(serde_json::to_value(crate::get_project_settings(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "update_project_settings" => typer(async {
            valeur(serde_json::to_value(crate::update_project_settings(etat,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
                serde_json::from_value(prendre(a, "composeFile", "compose_file"))
                    .map_err(|e| format!("argument composeFile : {e}"))?,
                serde_json::from_value(prendre(a, "description", "description"))
                    .map_err(|e| format!("argument description : {e}"))?,
                serde_json::from_value(prendre(a, "dependsOn", "depends_on"))
                    .map_err(|e| format!("argument dependsOn : {e}"))?,
            ).await?))
        })
        .await,
        "rename_project" => typer(async {
            valeur(serde_json::to_value(crate::rename_project(etat,
                serde_json::from_value(prendre(a, "oldName", "old_name"))
                    .map_err(|e| format!("argument oldName : {e}"))?,
                serde_json::from_value(prendre(a, "newName", "new_name"))
                    .map_err(|e| format!("argument newName : {e}"))?,
            ).await?))
        })
        .await,
        "get_system_metrics" => typer(async {
            valeur(serde_json::to_value(crate::get_system_metrics(etat, 
            ).await?))
        })
        .await,
        "kill_process" => typer(async {
            valeur(serde_json::to_value(crate::kill_process(etat,
                serde_json::from_value(prendre(a, "pid", "pid"))
                    .map_err(|e| format!("argument pid : {e}"))?,
            ).await?))
        })
        .await,
        "set_wallpaper" => typer(async {
            valeur(serde_json::to_value(crate::set_wallpaper(
                serde_json::from_value(prendre(a, "dataUrl", "data_url"))
                    .map_err(|e| format!("argument dataUrl : {e}"))?,
            ).await?))
        })
        .await,
        "get_wallpaper" => typer(async {
            valeur(serde_json::to_value(crate::get_wallpaper(
            ).await?))
        })
        .await,
        "clear_wallpaper" => typer(async {
            valeur(serde_json::to_value(crate::clear_wallpaper(
            ).await?))
        })
        .await,
        "read_image_as_data_url" => typer(async {
            valeur(serde_json::to_value(crate::read_image_as_data_url(
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
            ).await?))
        })
        .await,
        "import_database" => typer(async {
            valeur(serde_json::to_value(crate::import_database(etat,
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
            ).await?))
        })
        .await,
        "get_db_path" => typer(async {
            valeur(serde_json::to_value(crate::get_db_path(etat, 
            )))
        })
        .await,
        "start_recording" => typer(async {
            valeur(serde_json::to_value(crate::start_recording(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            ).await?))
        })
        .await,
        "stop_recording" => typer(async {
            valeur(serde_json::to_value(crate::stop_recording(etat, 
            ).await?))
        })
        .await,
        "get_active_recording" => typer(async {
            valeur(serde_json::to_value(crate::get_active_recording(etat, 
            )))
        })
        .await,
        "get_failed_recordings" => typer(async {
            valeur(serde_json::to_value(crate::get_failed_recordings(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            )?))
        })
        .await,
        "retry_recording" => typer(async {
            valeur(serde_json::to_value(crate::retry_recording(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "delete_recording" => typer(async {
            valeur(serde_json::to_value(crate::delete_recording(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "get_app_settings" => typer(async {
            valeur(serde_json::to_value(crate::get_app_settings(etat, 
            )?))
        })
        .await,
        "set_app_setting" => typer(async {
            valeur(serde_json::to_value(crate::set_app_setting(etat,
                serde_json::from_value(prendre(a, "key", "key"))
                    .map_err(|e| format!("argument key : {e}"))?,
                serde_json::from_value(prendre(a, "value", "value"))
                    .map_err(|e| format!("argument value : {e}"))?,
            )?))
        })
        .await,
        "get_project_summary_prompt" => typer(async {
            valeur(serde_json::to_value(crate::get_project_summary_prompt(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            ).await?))
        })
        .await,
        "set_project_summary_prompt" => typer(async {
            valeur(serde_json::to_value(crate::set_project_summary_prompt(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "prompt", "prompt"))
                    .map_err(|e| format!("argument prompt : {e}"))?,
            ).await?))
        })
        .await,
        "create_terminal" => typer(async {
            valeur(serde_json::to_value(crate::create_terminal(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "cwd", "cwd"))
                    .map_err(|e| format!("argument cwd : {e}"))?,
                serde_json::from_value(prendre(a, "cols", "cols"))
                    .map_err(|e| format!("argument cols : {e}"))?,
                serde_json::from_value(prendre(a, "rows", "rows"))
                    .map_err(|e| format!("argument rows : {e}"))?,
                serde_json::from_value(prendre(a, "initCommand", "init_command"))
                    .map_err(|e| format!("argument initCommand : {e}"))?,
            ).await?))
        })
        .await,
        "write_terminal" => typer(async {
            valeur(serde_json::to_value(crate::write_terminal(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "data", "data"))
                    .map_err(|e| format!("argument data : {e}"))?,
            )?))
        })
        .await,
        "resize_terminal" => typer(async {
            valeur(serde_json::to_value(crate::resize_terminal(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "cols", "cols"))
                    .map_err(|e| format!("argument cols : {e}"))?,
                serde_json::from_value(prendre(a, "rows", "rows"))
                    .map_err(|e| format!("argument rows : {e}"))?,
            ).await?))
        })
        .await,
        "close_terminal" => typer(async {
            valeur(serde_json::to_value(crate::close_terminal(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            ).await?))
        })
        .await,
        "save_terminal_screens" => typer(async {
            valeur(serde_json::to_value(crate::save_terminal_screens(etat, 
            ).await?))
        })
        .await,
        "attach_terminal" => typer(async {
            valeur(serde_json::to_value(crate::attach_terminal(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "cols", "cols"))
                    .map_err(|e| format!("argument cols : {e}"))?,
                serde_json::from_value(prendre(a, "rows", "rows"))
                    .map_err(|e| format!("argument rows : {e}"))?,
            ).await?))
        })
        .await,
        "rename_terminal" => typer(async {
            valeur(serde_json::to_value(crate::rename_terminal(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "list_terminals" => typer(async {
            valeur(serde_json::to_value(crate::list_terminals(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
            ).await?))
        })
        .await,
        "list_all_terminals" => typer(async {
            valeur(serde_json::to_value(crate::list_all_terminals(etat, 
            ).await?))
        })
        .await,
        "set_clipboard" => typer(async {
            valeur(serde_json::to_value(crate::set_clipboard(
                serde_json::from_value(prendre(a, "text", "text"))
                    .map_err(|e| format!("argument text : {e}"))?,
            )?))
        })
        .await,
        "get_clipboard" => typer(async {
            valeur(serde_json::to_value(crate::get_clipboard(
            )?))
        })
        .await,
        "llm_catalogue" => typer(async {
            valeur(serde_json::to_value(crate::llm_catalogue(etat, 
            )))
        })
        .await,
        "llm_choisir" => typer(async {
            valeur(serde_json::to_value(crate::llm_choisir(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "llm_poser_cle" => typer(async {
            valeur(serde_json::to_value(crate::llm_poser_cle(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "cle", "cle"))
                    .map_err(|e| format!("argument cle : {e}"))?,
            )?))
        })
        .await,
        "llm_conversations" => typer(async {
            valeur(serde_json::to_value(crate::llm_conversations(etat,
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            )?))
        })
        .await,
        "llm_renommer_conversation" => typer(async {
            valeur(serde_json::to_value(crate::llm_renommer_conversation(etat,
                serde_json::from_value(prendre(a, "conversationId", "conversation_id"))
                    .map_err(|e| format!("argument conversationId : {e}"))?,
                serde_json::from_value(prendre(a, "nom", "nom"))
                    .map_err(|e| format!("argument nom : {e}"))?,
            )?))
        })
        .await,
        "llm_commandes" => typer(async {
            valeur(serde_json::to_value(crate::llm_commandes(etat,
                serde_json::from_value(prendre(a, "conversationId", "conversation_id"))
                    .map_err(|e| format!("argument conversationId : {e}"))?,
            )?))
        })
        .await,
        "record_command" => typer(async {
            valeur(serde_json::to_value(crate::record_command(etat,
                serde_json::from_value(prendre(a, "project", "project"))
                    .map_err(|e| format!("argument project : {e}"))?,
                serde_json::from_value(prendre(a, "command", "command"))
                    .map_err(|e| format!("argument command : {e}"))?,
            )?))
        })
        .await,
        "terminal_search" => typer(async {
            valeur(serde_json::to_value(crate::terminal_search(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
                serde_json::from_value(prendre(a, "action", "action"))
                    .map_err(|e| format!("argument action : {e}"))?,
                serde_json::from_value(prendre(a, "query", "query"))
                    .map_err(|e| format!("argument query : {e}"))?,
            ).await?))
        })
        .await,
        "llm_reunions" => typer(async {
            valeur(serde_json::to_value(crate::llm_reunions(etat, 
            )))
        })
        .await,
        "llm_abonnement" => typer(async {
            valeur(serde_json::to_value(crate::llm_abonnement(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "llm_consommation" => typer(async {
            valeur(serde_json::to_value(crate::llm_consommation(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            ).await?))
        })
        .await,
        "llm_connexion_demarrer" => typer(async {
            valeur(serde_json::to_value(crate::llm_connexion_demarrer(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            )?))
        })
        .await,
        "llm_connexion_entrer" => typer(async {
            valeur(serde_json::to_value(crate::llm_connexion_entrer(etat,
                serde_json::from_value(prendre(a, "data", "data"))
                    .map_err(|e| format!("argument data : {e}"))?,
            )?))
        })
        .await,
        "llm_connexion_annuler" => typer(async {
            valeur(serde_json::to_value(crate::llm_connexion_annuler(etat, 
            )))
        })
        .await,
        "open_url" => typer(async {
            valeur(serde_json::to_value(crate::open_url(
                serde_json::from_value(prendre(a, "url", "url"))
                    .map_err(|e| format!("argument url : {e}"))?,
            )?))
        })
        .await,
        "report_error" => typer(async {
            valeur(serde_json::to_value(crate::report_error(etat,
                serde_json::from_value(prendre(a, "scope", "scope"))
                    .map_err(|e| format!("argument scope : {e}"))?,
                serde_json::from_value(prendre(a, "message", "message"))
                    .map_err(|e| format!("argument message : {e}"))?,
            ).await?))
        })
        .await,
        "machine_report" => typer(async {
            valeur(serde_json::to_value(crate::machine_report(
            ).await))
        })
        .await,
        "debug_log" => typer(async {
            valeur(serde_json::to_value(crate::debug_log(
                serde_json::from_value(prendre(a, "line", "line"))
                    .map_err(|e| format!("argument line : {e}"))?,
            ).await))
        })
        .await,
        "search_command_history" => typer(async {
            valeur(serde_json::to_value(crate::search_command_history(etat,
                serde_json::from_value(prendre(a, "query", "query"))
                    .map_err(|e| format!("argument query : {e}"))?,
                serde_json::from_value(prendre(a, "limit", "limit"))
                    .map_err(|e| format!("argument limit : {e}"))?,
            )))
        })
        .await,
        "list_project_dir" => typer(async {
            valeur(serde_json::to_value(crate::list_project_dir(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
            ).await?))
        })
        .await,
        "read_project_file" => typer(async {
            valeur(serde_json::to_value(crate::read_project_file(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
            ).await?))
        })
        .await,
        "stat_project_file" => typer(async {
            valeur(serde_json::to_value(crate::stat_project_file(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
            ).await?))
        })
        .await,
        "write_project_file" => typer(async {
            valeur(serde_json::to_value(crate::write_project_file(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
                serde_json::from_value(prendre(a, "content", "content"))
                    .map_err(|e| format!("argument content : {e}"))?,
            ).await?))
        })
        .await,
        "read_project_image" => typer(async {
            valeur(serde_json::to_value(crate::read_project_image(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
            ).await?))
        })
        .await,
        "backup_database" => typer(async {
            valeur(serde_json::to_value(crate::backup_database(etat,
                serde_json::from_value(prendre(a, "dest", "dest"))
                    .map_err(|e| format!("argument dest : {e}"))?,
            ).await?))
        })
        .await,
        "create_project_file" => typer(async {
            valeur(serde_json::to_value(crate::create_project_file(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relDir", "rel_dir"))
                    .map_err(|e| format!("argument relDir : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "create_project_dir" => typer(async {
            valeur(serde_json::to_value(crate::create_project_dir(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relDir", "rel_dir"))
                    .map_err(|e| format!("argument relDir : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "rename_project_entry" => typer(async {
            valeur(serde_json::to_value(crate::rename_project_entry(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
                serde_json::from_value(prendre(a, "newName", "new_name"))
                    .map_err(|e| format!("argument newName : {e}"))?,
            ).await?))
        })
        .await,
        "trash_project_entry" => typer(async {
            valeur(serde_json::to_value(crate::trash_project_entry(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
            ).await?))
        })
        .await,
        "search_project" => typer(async {
            valeur(serde_json::to_value(crate::search_project(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "query", "query"))
                    .map_err(|e| format!("argument query : {e}"))?,
            ).await?))
        })
        .await,
        "goto_definition" => typer(async {
            valeur(serde_json::to_value(crate::goto_definition(etat,
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "lang", "lang"))
                    .map_err(|e| format!("argument lang : {e}"))?,
                serde_json::from_value(prendre(a, "relPath", "rel_path"))
                    .map_err(|e| format!("argument relPath : {e}"))?,
                serde_json::from_value(prendre(a, "content", "content"))
                    .map_err(|e| format!("argument content : {e}"))?,
                serde_json::from_value(prendre(a, "line", "line"))
                    .map_err(|e| format!("argument line : {e}"))?,
                serde_json::from_value(prendre(a, "character", "character"))
                    .map_err(|e| format!("argument character : {e}"))?,
                serde_json::from_value(prendre(a, "symbol", "symbol"))
                    .map_err(|e| format!("argument symbol : {e}"))?,
            ).await?))
        })
        .await,
        "git_status" => typer(async {
            valeur(serde_json::to_value(crate::git_status(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            ).await?))
        })
        .await,
        "git_diff_file" => typer(async {
            valeur(serde_json::to_value(crate::git_diff_file(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
                serde_json::from_value(prendre(a, "untracked", "untracked"))
                    .map_err(|e| format!("argument untracked : {e}"))?,
            ).await?))
        })
        .await,
        "git_stage" => typer(async {
            valeur(serde_json::to_value(crate::git_stage(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
            ).await?))
        })
        .await,
        "git_unstage" => typer(async {
            valeur(serde_json::to_value(crate::git_unstage(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "path", "path"))
                    .map_err(|e| format!("argument path : {e}"))?,
            ).await?))
        })
        .await,
        "git_stage_all" => typer(async {
            valeur(serde_json::to_value(crate::git_stage_all(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            ).await?))
        })
        .await,
        "git_unstage_all" => typer(async {
            valeur(serde_json::to_value(crate::git_unstage_all(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            ).await?))
        })
        .await,
        "git_commit" => typer(async {
            valeur(serde_json::to_value(crate::git_commit(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "message", "message"))
                    .map_err(|e| format!("argument message : {e}"))?,
            ).await?))
        })
        .await,
        "git_push" => typer(async {
            valeur(serde_json::to_value(crate::git_push(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "setUpstream", "set_upstream"))
                    .map_err(|e| format!("argument setUpstream : {e}"))?,
            ).await?))
        })
        .await,
        "git_pull" => typer(async {
            valeur(serde_json::to_value(crate::git_pull(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            ).await?))
        })
        .await,
        "git_log" => typer(async {
            valeur(serde_json::to_value(crate::git_log(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "limit", "limit"))
                    .map_err(|e| format!("argument limit : {e}"))?,
            ).await?))
        })
        .await,
        "git_commit_diff" => typer(async {
            valeur(serde_json::to_value(crate::git_commit_diff(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "hash", "hash"))
                    .map_err(|e| format!("argument hash : {e}"))?,
            ).await?))
        })
        .await,
        "git_branches" => typer(async {
            valeur(serde_json::to_value(crate::git_branches(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            ).await?))
        })
        .await,
        "git_worktrees" => typer(async {
            valeur(serde_json::to_value(crate::git_worktrees(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            ).await?))
        })
        .await,
        "git_worktree_add" => typer(async {
            valeur(serde_json::to_value(crate::git_worktree_add(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "branche", "branche"))
                    .map_err(|e| format!("argument branche : {e}"))?,
                serde_json::from_value(prendre(a, "creer", "creer"))
                    .map_err(|e| format!("argument creer : {e}"))?,
            ).await?))
        })
        .await,
        "git_worktree_remove" => typer(async {
            valeur(serde_json::to_value(crate::git_worktree_remove(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "chemin", "chemin"))
                    .map_err(|e| format!("argument chemin : {e}"))?,
                serde_json::from_value(prendre(a, "force", "force"))
                    .map_err(|e| format!("argument force : {e}"))?,
            ).await?))
        })
        .await,
        "git_checkout_branch" => typer(async {
            valeur(serde_json::to_value(crate::git_checkout_branch(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "git_create_branch" => typer(async {
            valeur(serde_json::to_value(crate::git_create_branch(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            ).await?))
        })
        .await,
        "git_delete_branch" => typer(async {
            valeur(serde_json::to_value(crate::git_delete_branch(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "force", "force"))
                    .map_err(|e| format!("argument force : {e}"))?,
            ).await?))
        })
        .await,
        "get_marketplace_path" => typer(async {
            valeur(serde_json::to_value(crate::get_marketplace_path(
            )?))
        })
        .await,
        "list_marketplaces" => typer(async {
            valeur(serde_json::to_value(crate::list_marketplaces(
            )?))
        })
        .await,
        "list_plugins" => typer(async {
            valeur(serde_json::to_value(crate::list_plugins(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
            )?))
        })
        .await,
        "list_agents" => typer(async {
            valeur(serde_json::to_value(crate::list_agents(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
                serde_json::from_value(prendre(a, "plugin", "plugin"))
                    .map_err(|e| format!("argument plugin : {e}"))?,
            )?))
        })
        .await,
        "read_agent" => typer(async {
            valeur(serde_json::to_value(crate::read_agent(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
                serde_json::from_value(prendre(a, "plugin", "plugin"))
                    .map_err(|e| format!("argument plugin : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "save_agent" => typer(async {
            valeur(serde_json::to_value(crate::save_agent(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
                serde_json::from_value(prendre(a, "plugin", "plugin"))
                    .map_err(|e| format!("argument plugin : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "content", "content"))
                    .map_err(|e| format!("argument content : {e}"))?,
            )?))
        })
        .await,
        "delete_agent" => typer(async {
            valeur(serde_json::to_value(crate::delete_agent(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
                serde_json::from_value(prendre(a, "plugin", "plugin"))
                    .map_err(|e| format!("argument plugin : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "rename_agent" => typer(async {
            valeur(serde_json::to_value(crate::rename_agent(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
                serde_json::from_value(prendre(a, "plugin", "plugin"))
                    .map_err(|e| format!("argument plugin : {e}"))?,
                serde_json::from_value(prendre(a, "oldName", "old_name"))
                    .map_err(|e| format!("argument oldName : {e}"))?,
                serde_json::from_value(prendre(a, "newName", "new_name"))
                    .map_err(|e| format!("argument newName : {e}"))?,
            )?))
        })
        .await,
        "create_plugin" => typer(async {
            valeur(serde_json::to_value(crate::create_plugin(
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
                serde_json::from_value(prendre(a, "description", "description"))
                    .map_err(|e| format!("argument description : {e}"))?,
            )?))
        })
        .await,
        "delete_plugin" => typer(async {
            valeur(serde_json::to_value(crate::delete_plugin(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
                serde_json::from_value(prendre(a, "name", "name"))
                    .map_err(|e| format!("argument name : {e}"))?,
            )?))
        })
        .await,
        "rename_plugin" => typer(async {
            valeur(serde_json::to_value(crate::rename_plugin(
                serde_json::from_value(prendre(a, "marketplaceId", "marketplace_id"))
                    .map_err(|e| format!("argument marketplaceId : {e}"))?,
                serde_json::from_value(prendre(a, "oldName", "old_name"))
                    .map_err(|e| format!("argument oldName : {e}"))?,
                serde_json::from_value(prendre(a, "newName", "new_name"))
                    .map_err(|e| format!("argument newName : {e}"))?,
            )?))
        })
        .await,
        "get_project_plugins" => typer(async {
            valeur(serde_json::to_value(crate::get_project_plugins(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
            )?))
        })
        .await,
        "set_project_plugins" => typer(async {
            valeur(serde_json::to_value(crate::set_project_plugins(
                serde_json::from_value(prendre(a, "projectPath", "project_path"))
                    .map_err(|e| format!("argument projectPath : {e}"))?,
                serde_json::from_value(prendre(a, "plugins", "plugins"))
                    .map_err(|e| format!("argument plugins : {e}"))?,
            )?))
        })
        .await,
        "get_orchestrator_config" => typer(async {
            valeur(serde_json::to_value(crate::get_orchestrator_config(
            )?))
        })
        .await,
        "set_teams_enabled" => typer(async {
            valeur(serde_json::to_value(crate::set_teams_enabled(
                serde_json::from_value(prendre(a, "enabled", "enabled"))
                    .map_err(|e| format!("argument enabled : {e}"))?,
            )?))
        })
        .await,
        "set_teammate_mode" => typer(async {
            valeur(serde_json::to_value(crate::set_teammate_mode(
                serde_json::from_value(prendre(a, "mode", "mode"))
                    .map_err(|e| format!("argument mode : {e}"))?,
            )?))
        })
        .await,
        "toggle_plugin_enabled" => typer(async {
            valeur(serde_json::to_value(crate::toggle_plugin_enabled(
                serde_json::from_value(prendre(a, "pluginKey", "plugin_key"))
                    .map_err(|e| format!("argument pluginKey : {e}"))?,
                serde_json::from_value(prendre(a, "enabled", "enabled"))
                    .map_err(|e| format!("argument enabled : {e}"))?,
            )?))
        })
        .await,
        "compte_google_disponible" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_google_disponible(etat, 
            ).await?))
        })
        .await,
        "compte_google_direct" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_google_direct(etat, 
            ).await?))
        })
        .await,
        "compte_etat" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_etat(etat, 
            ).await?))
        })
        .await,
        "compte_inscription" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_inscription(etat,
                serde_json::from_value(prendre(a, "email", "email"))
                    .map_err(|e| format!("argument email : {e}"))?,
                serde_json::from_value(prendre(a, "motDePasse", "mot_de_passe"))
                    .map_err(|e| format!("argument motDePasse : {e}"))?,
                serde_json::from_value(prendre(a, "nom", "nom"))
                    .map_err(|e| format!("argument nom : {e}"))?,
            ).await?))
        })
        .await,
        "compte_connexion" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_connexion(etat,
                serde_json::from_value(prendre(a, "email", "email"))
                    .map_err(|e| format!("argument email : {e}"))?,
                serde_json::from_value(prendre(a, "motDePasse", "mot_de_passe"))
                    .map_err(|e| format!("argument motDePasse : {e}"))?,
            ).await?))
        })
        .await,
        "compte_connexion_google" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_connexion_google(etat, 
            ).await?))
        })
        .await,
        "compte_appairage_demarrer" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_appairage_demarrer(etat, 
            ).await?))
        })
        .await,
        "compte_appairage_etat" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_appairage_etat(etat,
                serde_json::from_value(prendre(a, "id", "id"))
                    .map_err(|e| format!("argument id : {e}"))?,
            ).await?))
        })
        .await,
        "compte_deconnexion" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_deconnexion(etat, 
            ).await?))
        })
        .await,
        "compte_machines" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_machines(etat, 
            ).await?))
        })
        .await,
        "compte_definir_nom" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_definir_nom(etat,
                serde_json::from_value(prendre(a, "nom", "nom"))
                    .map_err(|e| format!("argument nom : {e}"))?,
            ).await?))
        })
        .await,
        "compte_deposer_avatar" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_deposer_avatar(etat,
                serde_json::from_value(prendre(a, "chemin", "chemin"))
                    .map_err(|e| format!("argument chemin : {e}"))?,
            ).await?))
        })
        .await,
        "compte_lire_image" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_lire_image(
                serde_json::from_value(prendre(a, "chemin", "chemin"))
                    .map_err(|e| format!("argument chemin : {e}"))?,
            )?))
        })
        .await,
        "compte_deposer_image" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_deposer_image(etat,
                serde_json::from_value(prendre(a, "donnees", "donnees"))
                    .map_err(|e| format!("argument donnees : {e}"))?,
            ).await?))
        })
        .await,
        "compte_retirer_avatar" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_retirer_avatar(etat, 
            ).await?))
        })
        .await,
        "compte_definir_serveur" => typer(async {
            valeur(serde_json::to_value(crate::compte::compte_definir_serveur(etat,
                serde_json::from_value(prendre(a, "url", "url"))
                    .map_err(|e| format!("argument url : {e}"))?,
            ).await?))
        })
        .await,
        "synchro_maintenant" => typer(async {
            valeur(serde_json::to_value(crate::compte::synchro::synchro_maintenant(etat, 
            ).await?))
        })
        .await,
        "synchro_etat" => typer(async {
            valeur(serde_json::to_value(crate::compte::synchro::synchro_etat(etat, 
            ).await?))
        })
        .await,
        _ => return None,
    })
}
