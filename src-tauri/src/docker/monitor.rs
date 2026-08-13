use super::orchestrator::Orchestrator;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

pub async fn start_status_monitor(
    orch: Arc<Orchestrator>,
    interval_secs: u64,
    on_update: impl Fn() + Send + 'static,
) {
    // Initial refresh
    orch.refresh_statuses().await;
    on_update();

    let mut ticker = time::interval(Duration::from_secs(interval_secs));
    ticker.tick().await; // skip first immediate tick

    loop {
        ticker.tick().await;
        orch.refresh_statuses().await;
        on_update();
    }
}
