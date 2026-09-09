//! Lancer une tache de fond, avec ou sans Tauri.
//!
//! **POURQUOI UN SEUL ENDROIT.** `tauri::async_runtime::spawn` est un enrobage autour de
//! tokio, mais il pose SON runtime global : appeler `tokio::spawn` sous Tauri hors de ce
//! contexte panique, et appeler celui de Tauri sans Tauri ne compile pas. Sans ce module,
//! chaque appelant aurait invente sa propre condition, et l'un d'eux se serait trompe de
//! sens sans que rien ne le signale avant l'execution.

/// Lance une tache asynchrone qui ne rend rien.
pub fn lancer<F>(tache: F)
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    #[cfg(feature = "interface-tauri")]
    tauri::async_runtime::spawn(tache);
    #[cfg(not(feature = "interface-tauri"))]
    tokio::spawn(tache);
}

/// Lance un travail BLOQUANT sur un fil a lui, et rend de quoi attendre son resultat.
pub fn lancer_bloquant<F, T>(travail: F) -> impl std::future::Future<Output = Result<T, String>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    #[cfg(feature = "interface-tauri")]
    let attente = tauri::async_runtime::spawn_blocking(travail);
    #[cfg(not(feature = "interface-tauri"))]
    let attente = tokio::task::spawn_blocking(travail);
    async move { attente.await.map_err(|e| e.to_string()) }
}
