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

/// Kill a process by PID using the existing System instance from the Collector.
pub fn kill_process_with_sys(sys: &System, pid: u32) -> Result<(), String> {
    if pid <= 1 {
        return Err("cannot kill PID 0 or 1".to_string());
    }

    let sysinfo_pid = Pid::from_u32(pid);

    if let Some(process) = sys.process(sysinfo_pid) {
        if process.kill_with(Signal::Term).is_none() {
            return Err(format!("failed to send SIGTERM to PID {}", pid));
        }
        Ok(())
    } else {
        Err(format!("process {} not found", pid))
    }
}
