use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Disks, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disks: Vec<DiskMetrics>,
    pub hostname: String,
    /// Duree depuis le demarrage, en SECONDES : la mise en forme (« 3j 4h 12m ») est du
    /// texte affiche, elle appartient donc aux catalogues de traduction, pas au Rust.
    pub uptime_secs: u64,
    /// Le systeme et sa version, lisibles : « Ubuntu 26.04 », « Windows 11 »,
    /// « macOS 14.6 ». Remplace `kernel_version`, qui rendait « 22631 » sous Windows et
    /// « 23.6.0 » sous macOS — deux nombres que personne ne sait lire.
    pub os_version: String,
    /// Vrai quand la seule facon d'arreter un processus est de le TUER (voir
    /// `process::ARRET_FORCE`). L'interface en tire son libelle.
    pub kill_is_forced: bool,
    pub top_cpu: Vec<super::process::ProcessMetrics>,
    pub top_memory: Vec<super::process::ProcessMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub usage_percent: f64,
    pub cores: usize,
    pub model_name: String,
    pub per_core: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percent: f64,
    pub swap_total: u64,
    pub swap_used: u64,
    /// Le decoupage fin de la memoire, quand le systeme sait le donner — c'est-a-dire sous
    /// LINUX seulement.
    ///
    /// `sysinfo` n'expose que sept nombres de memoire et AUCUNE notion de cache, de
    /// buffers ou de partage, sur aucune plateforme : ce detail est du code natif par
    /// systeme, ou rien. Et les categories ne se traduisent pas d'un systeme a l'autre —
    /// macOS compte de la memoire *compressee* que Linux n'a pas, Windows n'a ni
    /// « buffers » ni « memoire partagee ». Le socle (total, utilise, swap) est donc commun
    /// aux trois, et ce supplement reste Linux : `None` ailleurs, et l'interface masque le
    /// panneau au lieu d'afficher cinq barres a zero.
    ///
    /// `None` aussi sous Linux quand `/proc/meminfo` est illisible : un echec de lecture ne
    /// doit pas fabriquer des zeros qui ressemblent a une mesure.
    pub detail: Option<MemoryDetail>,
}

/// Le detail memoire de Linux, lu dans `/proc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDetail {
    pub cached: u64,
    pub buffers: u64,
    pub shmem: u64,
    pub s_reclaimable: u64,
    pub zfs_arc: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub mount: String,
    pub device: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub percent: f64,
}

pub struct Collector {
    sys: System,
    disks: Disks,
    disk_refresh_counter: u8,
    /// Static system info cached once
    hostname: String,
    os_version: String,
    cpu_model: String,
    cpu_cores: usize,
}

impl Collector {
    pub fn new() -> Self {
        // Minimal init: only CPU info (for model/cores), no processes
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
        );
        sys.refresh_cpu_usage();

        let cpu_model = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();
        let cpu_cores = sys.cpus().len();

        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            disk_refresh_counter: 0,
            hostname: System::host_name().unwrap_or_default(),
            os_version: System::long_os_version().unwrap_or_default(),
            cpu_model,
            cpu_cores,
        }
    }

    pub fn collect(&mut self) -> SystemMetrics {
        // Targeted refreshes instead of refresh_all()
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes();

        let cpu = self.collect_cpu();
        let memory = self.collect_memory();
        let disks = self.collect_disks();
        let (top_cpu, top_memory) = super::process::collect_processes(&self.sys);

        SystemMetrics {
            cpu,
            memory,
            disks,
            hostname: self.hostname.clone(),
            uptime_secs: System::uptime(),
            os_version: self.os_version.clone(),
            kill_is_forced: super::process::ARRET_FORCE,
            top_cpu,
            top_memory,
        }
    }

    fn collect_cpu(&self) -> CpuMetrics {
        let per_core: Vec<f64> = self.sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect();
        let global = if per_core.is_empty() {
            0.0
        } else {
            per_core.iter().sum::<f64>() / per_core.len() as f64
        };

        CpuMetrics {
            usage_percent: global,
            cores: self.cpu_cores,
            model_name: self.cpu_model.clone(),
            per_core,
        }
    }

    fn collect_memory(&self) -> MemoryMetrics {
        let total = self.sys.total_memory();
        let available = self.sys.available_memory();
        let used = total.saturating_sub(available);
        let percent = if total > 0 {
            used as f64 / total as f64 * 100.0
        } else {
            0.0
        };

        MemoryMetrics {
            total,
            used,
            available,
            percent,
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            detail: lire_detail_memoire(),
        }
    }

    fn collect_disks(&mut self) -> Vec<DiskMetrics> {
        // Refresh disk list only every ~10 collects (~30s in live mode)
        self.disk_refresh_counter += 1;
        if self.disk_refresh_counter >= 10 {
            self.disk_refresh_counter = 0;
            self.disks = Disks::new_with_refreshed_list();
        }

        // PAS de filtre maison sur les points de montage. Les six chemins Unix qui etaient
        // ecrits ici ne matchaient RIEN sous Windows (`C:\`) et laissaient tomber
        // `/System/Volumes/Data` sous macOS, c'est-a-dire le volume ou sont les fichiers de
        // l'utilisateur : une liste de disques vide, sans message. `sysinfo` filtre deja, et
        // mieux, sur les trois systemes — il ecarte les pseudo-systemes de fichiers et les
        // montages snap sous Linux, les instantanes APFS et les volumes reseau sous macOS,
        // et ne garde que `DRIVE_FIXED`/`DRIVE_REMOVABLE` sous Windows. Effet de bord voulu
        // sous Linux : un disque monte sur `/mnt/data` ou `/srv` apparait enfin.
        self.disks
            .iter()
            .map(|d| {
                let total = d.total_space();
                let free = d.available_space();
                let used = total.saturating_sub(free);
                let percent = if total > 0 {
                    used as f64 / total as f64 * 100.0
                } else {
                    0.0
                };
                DiskMetrics {
                    mount: d.mount_point().to_string_lossy().to_string(),
                    // Sous Windows, `name()` est l'etiquette du volume et peut etre VIDE :
                    // l'affichage donnait « C:\ () ». Le frontend n'affiche rien plutot
                    // qu'une parenthese vide, il faut donc que le champ soit vraiment vide.
                    device: d.name().to_string_lossy().trim().to_string(),
                    total,
                    used,
                    free,
                    percent,
                }
            })
            .collect()
    }

    /// Access the inner System for kill_process reuse
    pub fn system(&self) -> &System {
        &self.sys
    }
}

/// Le detail memoire, Linux seulement (voir `MemoryMetrics::detail`).
#[cfg(target_os = "linux")]
fn lire_detail_memoire() -> Option<MemoryDetail> {
    let brut = std::fs::read_to_string("/proc/meminfo").ok()?;
    let (cached, buffers, shmem, s_reclaimable) = parse_proc_meminfo(&brut);
    Some(MemoryDetail {
        cached,
        buffers,
        shmem,
        s_reclaimable,
        zfs_arc: read_zfs_arc_size(),
    })
}

#[cfg(not(target_os = "linux"))]
fn lire_detail_memoire() -> Option<MemoryDetail> {
    None
}

/// Analyse separee de la lecture du fichier : c'est ce qui la rend testable.
#[cfg(target_os = "linux")]
fn parse_proc_meminfo(content: &str) -> (u64, u64, u64, u64) {
    let mut cached: u64 = 0;
    let mut buffers: u64 = 0;
    let mut shmem: u64 = 0;
    let mut s_reclaimable: u64 = 0;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let val_kb: u64 = parts[1].parse().unwrap_or(0);
        let val_bytes = val_kb * 1024;
        match parts[0] {
            "Cached:" => cached = val_bytes,
            "Buffers:" => buffers = val_bytes,
            "Shmem:" => shmem = val_bytes,
            "SReclaimable:" => s_reclaimable = val_bytes,
            _ => {}
        }
    }

    (cached, buffers, shmem, s_reclaimable)
}

#[cfg(target_os = "linux")]
fn read_zfs_arc_size() -> u64 {
    let content = match std::fs::read_to_string("/proc/spl/kstat/zfs/arcstats") {
        Ok(c) => c,
        Err(_) => return 0,
    };

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[0] == "size" {
            return parts[2].parse().unwrap_or(0);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les quatre lignes attendues sont lues, et les kilo-octets convertis en octets.
    #[test]
    #[cfg(target_os = "linux")]
    fn le_detail_memoire_se_lit_en_octets() {
        let brut = "MemTotal:       32000000 kB\n\
                    Buffers:            2048 kB\n\
                    Cached:          1024000 kB\n\
                    SReclaimable:     512000 kB\n\
                    Shmem:             65536 kB\n";
        let (cached, buffers, shmem, s_reclaimable) = parse_proc_meminfo(brut);
        assert_eq!(cached, 1_024_000 * 1024);
        assert_eq!(buffers, 2_048 * 1024);
        assert_eq!(shmem, 65_536 * 1024);
        assert_eq!(s_reclaimable, 512_000 * 1024);
    }

    /// Le detail n'existe que sous Linux : ailleurs l'interface masque le panneau, elle ne
    /// doit surtout pas recevoir des zeros qui passeraient pour une mesure.
    #[test]
    fn le_detail_memoire_est_reserve_a_linux() {
        assert_eq!(lire_detail_memoire().is_some(), cfg!(target_os = "linux"));
    }
}
