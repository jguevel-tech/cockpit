use super::compose::{Compose, ContainerStatus};
use super::graph::{self, Graph};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: String,
    pub description: String,
    pub depends_on: Vec<String>,
    pub depended_by: Vec<String>,
    pub state: ProjectState,
    pub containers: Vec<ContainerStatus>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
    /// Un fichier compose est-il utilisable pour ce projet ? Recalcule a chaque lecture
    /// (get_project/get_projects), jamais stocke : le frontend s'en sert pour ne pas offrir
    /// un Start qui ne peut pas aboutir, et l'info doit suivre l'ajout d'un docker-compose.yml
    /// sans redemarrage.
    pub has_compose: bool,
}

/// Prefixe des erreurs posees par le refresh de statuts — voir refresh_statuses.
const PS_ERROR_PREFIX: &str = "État Docker indisponible : ";

// --- Decisions pures (separees des locks et de docker pour etre testables) ---

/// Dependants encore actifs (Running/Starting) d'un projet — ils bloquent son arret.
fn active_dependants<F>(name: &str, reverse: &Graph, state_of: F) -> Vec<String>
where
    F: Fn(&str) -> Option<ProjectState>,
{
    reverse
        .get(name)
        .map(|dependants| {
            dependants
                .iter()
                .filter(|d| {
                    matches!(
                        state_of(d.as_str()),
                        Some(ProjectState::Running) | Some(ProjectState::Starting)
                    )
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Une dependance est orpheline si elle tourne et que plus aucun dependant n'est actif.
fn is_orphan_dep<F>(dep: &str, reverse: &Graph, state_of: F) -> bool
where
    F: Fn(&str) -> Option<ProjectState>,
{
    if !matches!(state_of(dep), Some(ProjectState::Running)) {
        return false;
    }
    active_dependants(dep, reverse, state_of).is_empty()
}

/// Transition d'etat calculee par le monitor a partir des conteneurs observes.
/// None = l'etat ne change pas. Un projet Stopped dont tous les conteneurs tournent est
/// adopte comme Running a TOUT moment, pas seulement au premier refresh : l'ancienne garde
/// `initial_done` laissait un projet cree en cours de session avec des conteneurs deja
/// lances afficher "stopped" jusqu'au redemarrage de l'app (constate chez le premier
/// utilisateur externe le 2026-08-14). Afficher la realite ne se perime pas.
fn refreshed_state(
    current: &ProjectState,
    has_containers: bool,
    all_running: bool,
) -> Option<ProjectState> {
    let was_active = matches!(current, ProjectState::Running | ProjectState::Error);
    if !has_containers {
        return if was_active { Some(ProjectState::Stopped) } else { None };
    }
    if all_running {
        Some(ProjectState::Running)
    } else {
        // Conteneurs partiellement up : on ne tranche pas, l'etat courant reste
        // (Error reste Error, Running reste Running, Stopped reste Stopped).
        None
    }
}

pub struct Orchestrator {
    projects: Arc<RwLock<HashMap<String, Project>>>,
    graph: Arc<RwLock<Graph>>,
    reverse: Arc<RwLock<Graph>>,
    composers: Arc<RwLock<HashMap<String, Compose>>>,
}

impl Orchestrator {
    pub fn new(
        project_defs: &[(String, String, String, String, Vec<String>)],
    ) -> Result<Self, String> {
        let graph_input: Vec<(String, Vec<String>)> = project_defs
            .iter()
            .map(|(name, _, _, _, deps)| (name.clone(), deps.clone()))
            .collect();

        let (dep_graph, rev_graph) = graph::build_graph(&graph_input);

        let (cycle, has_cycle) = graph::detect_cycles(&dep_graph);
        if has_cycle {
            return Err(format!(
                "dependency cycle detected: {}",
                graph::format_cycle(&cycle)
            ));
        }

        let mut projects = HashMap::new();
        let mut composers = HashMap::new();

        for (name, path, compose_file, description, deps) in project_defs {
            let depended_by = rev_graph.get(name).cloned().unwrap_or_default();
            projects.insert(
                name.clone(),
                Project {
                    name: name.clone(),
                    path: path.clone(),
                    description: description.clone(),
                    depends_on: deps.clone(),
                    depended_by,
                    state: ProjectState::Stopped,
                    containers: vec![],
                    error: String::new(),
                    has_compose: false,
                },
            );
            if !path.is_empty() {
                composers.insert(name.clone(), Compose::new(path, compose_file));
            }
        }

        Ok(Self {
            projects: Arc::new(RwLock::new(projects)),
            graph: Arc::new(RwLock::new(dep_graph)),
            reverse: Arc::new(RwLock::new(rev_graph)),
            composers: Arc::new(RwLock::new(composers)),
        })
    }

    pub async fn get_projects(&self) -> Vec<Project> {
        let projects = self.projects.read().await;
        let composers = self.composers.read().await;
        projects
            .values()
            .map(|p| {
                let mut p = p.clone();
                p.has_compose = composers.get(&p.name).is_some_and(|c| c.has_compose_file());
                p
            })
            .collect()
    }

    pub async fn get_project(&self, name: &str) -> Result<Project, String> {
        let projects = self.projects.read().await;
        let composers = self.composers.read().await;
        let mut proj = projects
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unknown project: {}", name))?;
        proj.has_compose = composers.get(name).is_some_and(|c| c.has_compose_file());
        Ok(proj)
    }

    pub async fn start_project(&self, name: &str) -> Result<(), String> {
        // Resolve startup order
        let graph = self.graph.read().await;
        let ordered = graph::topological_sort(&[name.to_string()], &graph)?;
        drop(graph);

        for proj_name in &ordered {
            // Check current state
            {
                let projects = self.projects.read().await;
                if let Some(p) = projects.get(proj_name) {
                    if p.state == ProjectState::Running {
                        continue;
                    }
                    if p.path.is_empty() {
                        continue;
                    }
                }
            }

            let composer = {
                let composers = self.composers.read().await;
                composers.get(proj_name).cloned()
            };

            // Refus AVANT de passer en "starting" : sans fichier compose, `docker compose up`
            // ne peut que repondre "no configuration file provided: not found", message qui
            // ne dit ni ou docker a cherche ni quoi faire. Le fichier compose etant optionnel
            // dans Cockpit, ce cas est normal et doit s'expliquer.
            if let Some(c) = &composer {
                if let Err(e) = c.require_compose_file() {
                    let msg = format!("Demarrage impossible pour {} : {}", proj_name, e);
                    let mut projects = self.projects.write().await;
                    if let Some(p) = projects.get_mut(proj_name) {
                        p.error = msg.clone();
                    }
                    return Err(msg);
                }
            }

            // Set state to starting
            {
                let mut projects = self.projects.write().await;
                if let Some(p) = projects.get_mut(proj_name) {
                    p.state = ProjectState::Starting;
                    p.error.clear();
                }
            }

            // Execute docker compose up (outside lock)
            if let Some(c) = composer {
                match c.up().await {
                    Ok(()) => {
                        let mut projects = self.projects.write().await;
                        if let Some(p) = projects.get_mut(proj_name) {
                            p.state = ProjectState::Running;
                        }
                    }
                    Err(e) => {
                        let mut projects = self.projects.write().await;
                        if let Some(p) = projects.get_mut(proj_name) {
                            p.state = ProjectState::Error;
                            p.error = e.clone();
                        }
                        return Err(format!("failed to start {}: {}", proj_name, e));
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn stop_project(&self, name: &str) -> Result<(), String> {
        // Check state and dependants
        {
            let projects = self.projects.read().await;
            let proj = projects
                .get(name)
                .ok_or_else(|| format!("unknown project: {}", name))?;

            if proj.state != ProjectState::Running && proj.state != ProjectState::Error {
                return Err(format!("project {} is not running (state: {:?})", name, proj.state));
            }

            let reverse = self.reverse.read().await;
            let active = active_dependants(name, &reverse, |n| {
                projects.get(n).map(|p| p.state.clone())
            });
            if !active.is_empty() {
                return Err(format!(
                    "cannot stop {}: active dependants: {}",
                    name,
                    active.join(", ")
                ));
            }
        }

        let composer = {
            let composers = self.composers.read().await;
            composers.get(name).cloned()
        };

        // Meme garde qu'au demarrage : les conteneurs peuvent avoir ete adoptes par leur label
        // (ps_by_working_dir) alors qu'aucun fichier compose n'est disponible ici — `down`
        // echouerait alors sur le message opaque de docker.
        if let Some(c) = &composer {
            if let Err(e) = c.require_compose_file() {
                let msg = format!(
                    "Arret impossible pour {} : {}. Les conteneurs deja lances peuvent etre \
                     arretes depuis la vue Conteneurs.",
                    name, e
                );
                let mut projects = self.projects.write().await;
                if let Some(p) = projects.get_mut(name) {
                    p.error = msg.clone();
                }
                return Err(msg);
            }
        }

        // Set stopping
        {
            let mut projects = self.projects.write().await;
            if let Some(p) = projects.get_mut(name) {
                p.state = ProjectState::Stopping;
                p.error.clear();
            }
        }

        // Execute docker compose down (outside lock)
        if let Some(c) = composer {
            match c.down().await {
                Ok(()) => {
                    let mut projects = self.projects.write().await;
                    if let Some(p) = projects.get_mut(name) {
                        p.state = ProjectState::Stopped;
                        p.containers.clear();
                    }
                }
                Err(e) => {
                    let mut projects = self.projects.write().await;
                    if let Some(p) = projects.get_mut(name) {
                        p.state = ProjectState::Error;
                        p.error = e.clone();
                    }
                    return Err(e);
                }
            }
        }

        // Cleanup orphan dependencies
        self.cleanup_orphan_deps(name).await;

        Ok(())
    }

    pub async fn restart_project(&self, name: &str) -> Result<(), String> {
        let is_running = {
            let projects = self.projects.read().await;
            projects.get(name).map_or(false, |p| {
                p.state == ProjectState::Running || p.state == ProjectState::Error
            })
        };

        if is_running {
            self.stop_project(name).await?;
        }
        self.start_project(name).await
    }

    async fn cleanup_orphan_deps(&self, name: &str) {
        let deps = {
            let graph = self.graph.read().await;
            graph.get(name).cloned().unwrap_or_default()
        };

        for dep in &deps {
            let should_stop = {
                let projects = self.projects.read().await;
                let reverse = self.reverse.read().await;
                is_orphan_dep(dep, &reverse, |n| projects.get(n).map(|p| p.state.clone()))
            };

            if should_stop {
                // Set stopping
                {
                    let mut projects = self.projects.write().await;
                    if let Some(p) = projects.get_mut(dep.as_str()) {
                        p.state = ProjectState::Stopping;
                    }
                }

                let composer = {
                    let composers = self.composers.read().await;
                    composers.get(dep.as_str()).cloned()
                };

                if let Some(c) = composer {
                    match c.down().await {
                        Ok(()) => {
                            let mut projects = self.projects.write().await;
                            if let Some(p) = projects.get_mut(dep.as_str()) {
                                p.state = ProjectState::Stopped;
                                p.containers.clear();
                            }
                        }
                        Err(e) => {
                            let mut projects = self.projects.write().await;
                            if let Some(p) = projects.get_mut(dep.as_str()) {
                                p.state = ProjectState::Error;
                                p.error = e;
                            }
                        }
                    }
                }

                // Recursively cleanup
                Box::pin(self.cleanup_orphan_deps(dep)).await;
            }
        }
    }

    pub async fn add_project(
        &self,
        name: &str,
        path: &str,
        compose_file: &str,
        description: &str,
        depends_on: Vec<String>,
    ) -> Result<(), String> {
        let mut projects = self.projects.write().await;
        if projects.contains_key(name) {
            return Err(format!("project {} already exists", name));
        }

        let mut graph = self.graph.write().await;
        let mut reverse = self.reverse.write().await;

        graph.insert(name.to_string(), depends_on.clone());
        for dep in &depends_on {
            reverse.entry(dep.clone()).or_default().push(name.to_string());
        }

        let depended_by = reverse.get(name).cloned().unwrap_or_default();

        projects.insert(
            name.to_string(),
            Project {
                name: name.to_string(),
                path: path.to_string(),
                description: description.to_string(),
                depends_on,
                depended_by,
                state: ProjectState::Stopped,
                containers: vec![],
                error: String::new(),
                has_compose: false,
            },
        );

        if !path.is_empty() {
            let mut composers = self.composers.write().await;
            composers.insert(name.to_string(), Compose::new(path, compose_file));
        }

        Ok(())
    }

    pub async fn update_project(
        &self,
        name: &str,
        path: &str,
        compose_file: &str,
        description: &str,
        depends_on: Vec<String>,
    ) {
        let mut projects = self.projects.write().await;
        if let Some(proj) = projects.get_mut(name) {
            proj.path = path.to_string();
            proj.description = description.to_string();
            proj.depends_on = depends_on;
        }

        let mut composers = self.composers.write().await;
        if !path.is_empty() {
            composers.insert(name.to_string(), Compose::new(path, compose_file));
        } else {
            composers.remove(name);
        }
    }

    pub async fn rename_project(&self, old_name: &str, new_name: &str) {
        let mut projects = self.projects.write().await;
        if let Some(mut proj) = projects.remove(old_name) {
            proj.name = new_name.to_string();
            projects.insert(new_name.to_string(), proj);
        }

        let mut graph = self.graph.write().await;
        if let Some(deps) = graph.remove(old_name) {
            graph.insert(new_name.to_string(), deps);
        }
        // Update references in other projects' deps
        for deps in graph.values_mut() {
            for dep in deps.iter_mut() {
                if dep == old_name { *dep = new_name.to_string(); }
            }
        }

        let mut reverse = self.reverse.write().await;
        if let Some(dependants) = reverse.remove(old_name) {
            reverse.insert(new_name.to_string(), dependants);
        }
        for dependants in reverse.values_mut() {
            for d in dependants.iter_mut() {
                if d == old_name { *d = new_name.to_string(); }
            }
        }

        // Update depended_by in projects
        for proj in projects.values_mut() {
            for d in proj.depended_by.iter_mut() {
                if d == old_name { *d = new_name.to_string(); }
            }
            for d in proj.depends_on.iter_mut() {
                if d == old_name { *d = new_name.to_string(); }
            }
        }

        let mut composers = self.composers.write().await;
        if let Some(c) = composers.remove(old_name) {
            composers.insert(new_name.to_string(), c);
        }
    }

    pub async fn remove_project(&self, name: &str) -> Result<(), String> {
        let mut projects = self.projects.write().await;
        let proj = projects
            .get(name)
            .ok_or_else(|| format!("unknown project: {}", name))?;

        if proj.state == ProjectState::Running || proj.state == ProjectState::Starting {
            return Err(format!("cannot remove {}: project is active", name));
        }

        projects.remove(name);

        let mut graph = self.graph.write().await;
        let mut reverse = self.reverse.write().await;
        let mut composers = self.composers.write().await;

        graph.remove(name);
        reverse.remove(name);
        for (_, dependants) in reverse.iter_mut() {
            dependants.retain(|d| d != name);
        }
        composers.remove(name);

        Ok(())
    }

    pub async fn refresh_statuses(&self) {
        // Phase 1: collect targets under read lock
        let targets: Vec<(String, ProjectState)> = {
            let projects = self.projects.read().await;
            projects
                .values()
                .filter(|p| p.state != ProjectState::Starting && p.state != ProjectState::Stopping)
                .map(|p| (p.name.clone(), p.state.clone()))
                .collect()
        };

        // Phase 2: execute docker compose ps outside lock
        let mut results: Vec<(String, Result<Vec<ContainerStatus>, String>)> = Vec::new();
        for (name, _) in &targets {
            let composer = {
                let composers = self.composers.read().await;
                composers.get(name).cloned()
            };
            match composer {
                Some(c) if c.has_compose_file() => {
                    let res = c.ps().await;
                    results.push((name.clone(), res));
                }
                // Un chemin sans fichier compose au nom standard : docker retrouve quand
                // meme les conteneurs compose lances depuis ce dossier via leur label.
                Some(c) => {
                    let res = super::compose::ps_by_working_dir(&c.project_dir).await;
                    results.push((name.clone(), res));
                }
                None => {
                    results.push((name.clone(), Ok(vec![])));
                }
            }
        }

        // Phase 3: apply results under write lock
        let mut projects = self.projects.write().await;

        for (name, result) in &results {
            let proj = match projects.get_mut(name.as_str()) {
                Some(p) => p,
                None => continue,
            };

            if proj.state == ProjectState::Starting || proj.state == ProjectState::Stopping {
                continue;
            }

            // Une erreur d'observation (docker.sock inaccessible, docker absent...) doit
            // s'afficher au lieu de laisser croire a un projet arrete. Le prefixe sert de
            // marqueur : on n'efface au refresh suivant QUE nos propres messages, jamais
            // une erreur posee par un start/stop que l'utilisateur n'a pas encore lue.
            let containers = match result {
                Ok(c) => {
                    if proj.error.starts_with(PS_ERROR_PREFIX) {
                        proj.error.clear();
                    }
                    c
                }
                Err(e) => {
                    proj.error = format!("{}{}", PS_ERROR_PREFIX, e);
                    continue;
                }
            };

            proj.containers = containers.clone();

            let all_running = containers.iter().all(|c| c.status == "running");
            if let Some(new_state) =
                refreshed_state(&proj.state, !containers.is_empty(), all_running)
            {
                proj.state = new_state;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn states(pairs: &[(&str, ProjectState)]) -> HashMap<String, ProjectState> {
        pairs.iter().map(|(n, s)| (n.to_string(), s.clone())).collect()
    }

    fn rev(pairs: &[(&str, &[&str])]) -> Graph {
        pairs
            .iter()
            .map(|(n, deps)| (n.to_string(), deps.iter().map(|d| d.to_string()).collect()))
            .collect()
    }

    #[test]
    fn stop_blocked_by_running_dependant() {
        let s = states(&[("db", ProjectState::Running), ("web", ProjectState::Running)]);
        let r = rev(&[("db", &["web"])]);
        let active = active_dependants("db", &r, |n| s.get(n).cloned());
        assert_eq!(active, vec!["web".to_string()]);
    }

    #[test]
    fn stop_allowed_when_dependants_stopped() {
        let s = states(&[("db", ProjectState::Running), ("web", ProjectState::Stopped)]);
        let r = rev(&[("db", &["web"])]);
        assert!(active_dependants("db", &r, |n| s.get(n).cloned()).is_empty());
    }

    #[test]
    fn starting_dependant_also_blocks() {
        let s = states(&[("db", ProjectState::Running), ("web", ProjectState::Starting)]);
        let r = rev(&[("db", &["web"])]);
        assert_eq!(active_dependants("db", &r, |n| s.get(n).cloned()).len(), 1);
    }

    #[test]
    fn orphan_dep_running_without_active_dependants() {
        let s = states(&[("db", ProjectState::Running), ("web", ProjectState::Stopped)]);
        let r = rev(&[("db", &["web"])]);
        assert!(is_orphan_dep("db", &r, |n| s.get(n).cloned()));
    }

    #[test]
    fn dep_not_orphan_if_stopped_or_still_used() {
        let r = rev(&[("db", &["web"])]);
        // Deja arretee : rien a faire
        let s = states(&[("db", ProjectState::Stopped), ("web", ProjectState::Stopped)]);
        assert!(!is_orphan_dep("db", &r, |n| s.get(n).cloned()));
        // Encore utilisee : ne pas stopper
        let s = states(&[("db", ProjectState::Running), ("web", ProjectState::Running)]);
        assert!(!is_orphan_dep("db", &r, |n| s.get(n).cloned()));
    }

    #[test]
    fn orphan_dep_without_reverse_entry_is_orphan() {
        let s = states(&[("db", ProjectState::Running)]);
        let r = rev(&[]);
        assert!(is_orphan_dep("db", &r, |n| s.get(n).cloned()));
    }

    #[test]
    fn refresh_running_without_containers_becomes_stopped() {
        assert_eq!(
            refreshed_state(&ProjectState::Running, false, true),
            Some(ProjectState::Stopped)
        );
        assert_eq!(
            refreshed_state(&ProjectState::Error, false, true),
            Some(ProjectState::Stopped)
        );
    }

    #[test]
    fn refresh_stopped_without_containers_stays() {
        assert_eq!(refreshed_state(&ProjectState::Stopped, false, true), None);
    }

    #[test]
    fn refresh_error_recovers_when_all_running() {
        assert_eq!(
            refreshed_state(&ProjectState::Error, true, true),
            Some(ProjectState::Running)
        );
        // Conteneurs partiellement up : on reste en erreur
        assert_eq!(refreshed_state(&ProjectState::Error, true, false), None);
    }

    #[test]
    fn refresh_adopts_running_containers_anytime() {
        // Un projet Stopped dont les conteneurs tournent est adopte comme Running,
        // y compris longtemps apres le demarrage de l'app : c'est le cas d'un projet
        // cree en cours de session alors que ses conteneurs tournaient deja.
        assert_eq!(
            refreshed_state(&ProjectState::Stopped, true, true),
            Some(ProjectState::Running)
        );
        // Partiellement up : on n'invente pas un etat
        assert_eq!(refreshed_state(&ProjectState::Stopped, true, false), None);
    }

    fn orch_with(path: &str) -> Orchestrator {
        Orchestrator::new(&[(
            "demo".to_string(),
            path.to_string(),
            String::new(),
            String::new(),
            vec![],
        )])
        .unwrap()
    }

    #[tokio::test]
    async fn start_refuse_sans_fichier_compose_avec_un_message_utile() {
        let dir = std::env::temp_dir().join(format!("cockpit_orch_nocompose_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let orch = orch_with(dir.to_str().unwrap());

        let err = orch.start_project("demo").await.unwrap_err();
        assert!(
            err.contains(dir.to_str().unwrap()) && err.contains("docker-compose.yml"),
            "message opaque : {}",
            err
        );
        // Aucune commande docker n'a tourne : l'etat ne reste pas coince en "starting",
        // et l'erreur est visible dans l'onglet Docker.
        let p = orch.get_project("demo").await.unwrap();
        assert_eq!(p.state, ProjectState::Stopped);
        assert!(!p.error.is_empty());
        assert!(!p.has_compose);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn has_compose_suit_l_apparition_du_fichier() {
        let dir = std::env::temp_dir().join(format!("cockpit_orch_compose_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let orch = orch_with(dir.to_str().unwrap());
        assert!(!orch.get_project("demo").await.unwrap().has_compose);

        // Le fichier ajoute apres coup doit etre vu sans redemarrer l'app.
        std::fs::write(dir.join("docker-compose.yml"), "services: {}\n").unwrap();
        assert!(orch.get_project("demo").await.unwrap().has_compose);
        assert!(orch.get_projects().await[0].has_compose);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn refresh_never_touches_transitional_states_here() {
        // Starting/Stopping sont filtres en amont par refresh_statuses ;
        // la fonction pure ne les promeut pas si partiellement up.
        assert_eq!(refreshed_state(&ProjectState::Starting, true, false), None);
        assert_eq!(refreshed_state(&ProjectState::Stopping, false, false), None);
    }
}
