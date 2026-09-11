//! Lancer une tache de fond, en un seul endroit.
//!
//! **POURQUOI UN SEUL ENDROIT.** Il y a eu deux runtimes a une epoque (celui de Tauri et
//! tokio), et chaque appelant inventait sa propre condition pour choisir. L'un d'eux se
//! serait trompe de sens sans que rien ne le signale avant l'execution. Il n'en reste qu'un,
//! et ce module garde la trace unique de ce choix.

/// Lance une tache asynchrone qui ne rend rien.
pub fn lancer<F>(tache: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    tokio::spawn(tache);
}

/// Lance un travail BLOQUANT sur un fil a lui, et rend de quoi attendre son resultat.
pub fn lancer_bloquant<F, T>(travail: F) -> impl std::future::Future<Output = Result<T, String>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let attente = tokio::task::spawn_blocking(travail);
    async move { attente.await.map_err(|e| e.to_string()) }
}
