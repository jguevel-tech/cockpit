use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Disks, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disks: Vec<DiskMetrics>,
    pub hostname: String,
    pub uptime: String,
    pub kernel_version: String,
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
    kernel_version: String,
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
            kernel_version: System::kernel_version().unwrap_or_default(),
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
            uptime: format_uptime(System::uptime()),
            kernel_version: self.kernel_version.clone(),
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

        let (cached, buffers, shmem, s_reclaimable) = read_proc_meminfo();
        let zfs_arc = read_zfs_arc_size();

        MemoryMetrics {
            total,
            used,
            available,
            percent,
            swap_total: self.sys.total_swap(),
            swap_used: self.sys.used_swap(),
            cached,
            buffers,
            shmem,
            s_reclaimable,
            zfs_arc,
        }
    }

    fn collect_disks(&mut self) -> Vec<DiskMetrics> {
        // Refresh disk list only every ~10 collects (~30s in live mode)
        self.disk_refresh_counter += 1;
        if self.disk_refresh_counter >= 10 {
            self.disk_refresh_counter = 0;
            self.disks = Disks::new_with_refreshed_list();
        }

        self.disks
            .iter()
            .filter(|d| {
                let mount = d.mount_point().to_string_lossy();
                matches!(
                    mount.as_ref(),
                    "/" | "/home" | "/boot" | "/var" | "/tmp" | "/opt"
                )
            })
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
                    device: d.name().to_string_lossy().to_string(),
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

fn read_proc_meminfo() -> (u64, u64, u64, u64) {
    let content = match std::fs::read_to_string("/proc/meminfo") {
        Ok(c) => c,
        Err(_) => return (0, 0, 0, 0),
    };

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

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}j {}h {}m", days, hours, mins)
    } else {
        format!("{}h {}m", hours, mins)
    }
}
