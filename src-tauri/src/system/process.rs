use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use sysinfo::{Pid, Signal, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMetrics {
    pub pid: u32,
    pub name: String,
    pub cpu: f64,
    pub memory: f64,
    pub memory_rss: u64,
    pub user: String,
    pub command: String,
    pub count: Option<usize>,
    pub children: Option<Vec<ProcessMetrics>>,
}

pub fn collect_processes(sys: &System) -> (Vec<ProcessMetrics>, Vec<ProcessMetrics>) {
    let total_memory = sys.total_memory() as f64;

    let mut all: Vec<ProcessMetrics> = sys
        .processes()
        .values()
        .map(|p| {
            let rss = p.memory();
            let mem_pct = if total_memory > 0.0 {
                rss as f64 / total_memory * 100.0
            } else {
                0.0
            };
            ProcessMetrics {
                pid: p.pid().as_u32(),
                name: p.name().to_string(),
                cpu: p.cpu_usage() as f64,
                memory: mem_pct,
                memory_rss: rss,
                user: p
                    .user_id()
                    .map(|u| u.to_string())
                    .unwrap_or_default(),
                command: p.cmd().join(" "),
                count: None,
                children: None,
            }
        })
        .collect();

    // Top CPU
    all.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    let top_cpu: Vec<_> = all.iter().take(20).cloned().collect();

    // Group by name for memory
    let mut groups: HashMap<String, Vec<ProcessMetrics>> = HashMap::new();
    for p in &all {
        groups.entry(p.name.clone()).or_default().push(p.clone());
    }

    let mut top_memory: Vec<ProcessMetrics> = groups
        .into_iter()
        .map(|(name, mut procs)| {
            procs.sort_by(|a, b| b.memory.partial_cmp(&a.memory).unwrap_or(std::cmp::Ordering::Equal));
            let total_cpu: f64 = procs.iter().map(|p| p.cpu).sum();
            let total_mem: f64 = procs.iter().map(|p| p.memory).sum();
            let total_rss: u64 = procs.iter().map(|p| p.memory_rss).sum();
            let count = procs.len();
            let top = &procs[0];

            ProcessMetrics {
                pid: top.pid,
                name: name.clone(),
                cpu: total_cpu,
                memory: total_mem,
                memory_rss: total_rss,
                user: top.user.clone(),
                command: top.command.clone(),
                count: Some(count),
                children: if count > 1 { Some(procs) } else { None },
            }
        })
        .collect();

    top_memory.sort_by(|a, b| b.memory.partial_cmp(&a.memory).unwrap_or(std::cmp::Ordering::Equal));
    top_memory.truncate(20);

    (top_cpu, top_memory)
}

/// L'arret d'un processus est-il FORCE sur ce systeme ?
///
/// Sous Windows, oui, et il n'y a pas de choix : `sysinfo` n'accepte que `Signal::Kill`
/// (`windows/mod.rs`, la branche `_ => None` de la conversion) et l'applique par
/// `taskkill.exe /F`. Le piege est SILENCIEUX a la compilation — `kill_with(Signal::Term)`
/// compile partout, c'est la conversion qui rend `None` a l'execution, et notre message
/// d'erreur nommait alors « SIGTERM », un mecanisme qui n'existe pas sur ce systeme.
/// L'interface s'en sert pour son libelle : « Forcer l'arret » et pas « Arreter », sinon un
/// utilisateur habitue a Cockpit sous Linux croira que son editeur va pouvoir sauvegarder.
pub const ARRET_FORCE: bool = cfg!(windows);

/// Le signal envoye : le plus doux que le systeme accepte.
#[cfg(not(windows))]
const SIGNAL_D_ARRET: Signal = Signal::Term;
#[cfg(windows)]
const SIGNAL_D_ARRET: Signal = Signal::Kill;

/// PIDs qu'on refuse de toucher, quoi qu'il arrive.
///
/// 0 et 1 sont refuses partout (`init`/`launchd` sous Unix, « System Idle Process » sous
/// Windows). 4 est le processus `System` de Windows : sans ce refus, un clic sur la premiere
/// ligne du tableau partait en `taskkill /F` sur le noyau. Le refus doit etre EXPLIQUE, pas
/// confie au hasard des permissions.
fn pid_intouchable(pid: u32) -> Option<&'static str> {
    match pid {
        0 | 1 => Some("les PID 0 et 1 appartiennent au systeme"),
        #[cfg(windows)]
        4 => Some("le PID 4 est le processus System de Windows"),
        _ => None,
    }
}

/// Kill a process by PID using the existing System instance from the Collector.
pub fn kill_process_with_sys(sys: &System, pid: u32) -> Result<(), String> {
    if let Some(raison) = pid_intouchable(pid) {
        return Err(format!("PID {pid} : {raison}"));
    }

    let sysinfo_pid = Pid::from_u32(pid);

    if let Some(process) = sys.process(sysinfo_pid) {
        if process.kill_with(SIGNAL_D_ARRET).is_none() {
            return Err(format!(
                "signal {SIGNAL_D_ARRET:?} refuse par le systeme pour le PID {pid}"
            ));
        }
        Ok(())
    } else {
        Err(format!("process {} not found", pid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le refus doit DIRE pourquoi : un « cannot kill » sec envoyait chercher un probleme de
    /// permission la ou il y a une regle.
    #[test]
    fn les_pids_du_systeme_sont_refuses_avec_une_raison() {
        for pid in [0, 1] {
            let raison = pid_intouchable(pid).unwrap_or_else(|| panic!("PID {pid} accepte"));
            assert!(raison.contains("systeme"), "{raison}");
        }
    }

    /// 4 est le processus `System` de Windows ; sous Unix c'est un fil du noyau ordinaire
    /// qu'on n'a aucune raison de traiter a part.
    #[test]
    fn le_pid_4_ne_depend_que_de_windows() {
        assert_eq!(pid_intouchable(4).is_some(), cfg!(windows));
    }

    #[test]
    fn un_pid_ordinaire_passe() {
        assert!(pid_intouchable(std::process::id()).is_none());
    }

    /// Le libelle de l'interface se decide sur cette constante : elle doit suivre la
    /// plateforme, sinon Windows affiche « Arreter » pour un `taskkill /F`.
    #[test]
    fn l_arret_est_force_uniquement_sous_windows() {
        assert_eq!(ARRET_FORCE, cfg!(windows));
        assert_eq!(SIGNAL_D_ARRET == Signal::Kill, cfg!(windows));
    }
}
