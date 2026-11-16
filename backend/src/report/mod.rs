//! Journal local des erreurs (`<donnees>/logs/cockpit.log`).
//!
//! **RIEN NE QUITTE LA MACHINE.** L'envoi vers un serveur de suivi a existe ; il est retire,
//! avec le serveur. Le journal reste : il marche hors ligne, et c'est lui que l'utilisateur
//! peut relire ou joindre a un signalement.

use std::path::{Path, PathBuf};

const LOG_NAME: &str = "cockpit.log";
/// Au-dela, le journal est reparti dans un `.1` : on garde de quoi diagnostiquer sans
/// laisser un fichier grossir sans fin.
const LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

fn log_path(app_data: &Path) -> PathBuf {
    app_data.join("logs").join(LOG_NAME)
}

/// Ecrit une ligne dans le journal local, en le faisant tourner s'il est trop gros.
pub fn append_log(app_data: &Path, ligne: &str) {
    let chemin = log_path(app_data);
    if let Some(parent) = chemin.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::metadata(&chemin).map(|m| m.len()).unwrap_or(0) > LOG_MAX_BYTES {
        let _ = std::fs::rename(&chemin, chemin.with_extension("log.1"));
    }
    // Le journal ne doit jamais faire echouer ce qu'il observe : une ecriture impossible
    // (disque plein, droits) est ignoree.
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&chemin) {
        let _ = writeln!(f, "{ligne}");
    }
}

/// Ligne de journal : horodatee, sur une seule ligne pour rester lisible et greppable.
pub fn format_log_line(horodatage: &str, scope: &str, message: &str) -> String {
    format!(
        "{horodatage} [{scope}] {}",
        message.replace('\n', " ").replace('\r', " ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ligne_de_journal_tient_sur_une_ligne() {
        let ligne = format_log_line("2026-08-19 14:00:00", "git.commit", "echec\nsur deux lignes");
        assert!(!ligne.trim_end().contains('\n'), "{ligne}");
        assert!(ligne.contains("[git.commit]"), "{ligne}");
    }
}
