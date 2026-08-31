//! Le mode de rendu de la vue web, decide AVANT l'initialisation de GTK.
//!
//! **Le probleme, mesure le 2026-08-31.** Le journal du guetteur montre des episodes ou la page
//! rend compte normalement mais ne peint AUCUNE image : le backend va bien, la boucle graphique
//! repond, et la fenetre est morte a l'ecran. Il faut tuer l'application pour en sortir.
//!
//! C'est un defaut connu de WebKitGTK avec le pilote proprietaire NVIDIA : le chemin de rendu
//! par DMA-BUF que le moteur prefere n'y fonctionne pas de facon fiable. Le rapport amont
//! s'intitule « [GTK] Disable DMABuf renderer for NVIDIA proprietary drivers », et le
//! contournement partage par tout l'ecosysteme est une variable d'environnement posee avant que
//! le moteur ne demarre. Machine concernee ici : RTX 500 Ada avec le pilote `nvidia`, session
//! Wayland, WebKitGTK 2.52.
//!
//! **On ne le coupe pas partout.** Sur Intel et AMD ce chemin marche et il est plus rapide : le
//! desactiver pour tout le monde ferait payer a tous le defaut d'un seul pilote. La condition
//! est donc la presence du pilote proprietaire, qu'on lit dans `/sys`.
//!
//! **Et on n'ecrase jamais un choix de l'utilisateur** : qui a pose la variable lui-meme a
//! peut-etre une bonne raison, dans un sens comme dans l'autre.

/// La variable que WebKitGTK lit au demarrage. Elle n'existe que sous Linux : ce moteur n'est
/// pas celui des autres systemes.
#[cfg(target_os = "linux")]
const VARIABLE: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// Ce qui a ete decide, pour que le journal le dise au lieu de le laisser deviner.
static MODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Le pilote proprietaire NVIDIA est-il charge ?
///
/// `/sys/module/nvidia` n'existe qu'avec le pilote proprietaire : ni avec nouveau, ni sur une
/// machine sans carte NVIDIA. Une carte presente mais inutilisee n'a pas d'importance — c'est le
/// pilote qui fournit le chemin de rendu fautif.
#[cfg(target_os = "linux")]
fn pilote_nvidia_proprietaire() -> bool {
    std::path::Path::new("/sys/module/nvidia").exists()
}

/// **A appeler AVANT toute initialisation de GTK.** WebKitGTK lit cette variable au demarrage du
/// moteur : la poser apres n'a aucun effet, et l'echec serait silencieux.
#[cfg(target_os = "linux")]
pub fn decider() {
    let mode = if let Ok(deja) = std::env::var(VARIABLE) {
        format!("rendu : choix de l'utilisateur conserve ({VARIABLE}={deja})")
    } else if pilote_nvidia_proprietaire() {
        std::env::set_var(VARIABLE, "1");
        "rendu : DMA-BUF desactive (pilote NVIDIA proprietaire detecte)".to_string()
    } else {
        "rendu : DMA-BUF laisse actif (pas de pilote NVIDIA proprietaire)".to_string()
    };
    let _ = MODE.set(mode);
}

#[cfg(not(target_os = "linux"))]
pub fn decider() {
    // Ce chemin de rendu n'existe que dans WebKitGTK : rien a decider ailleurs.
    let _ = MODE.set("rendu : sans objet sur ce systeme".to_string());
}

/// Ce qui a ete decide, pour le journal.
pub fn mode() -> String {
    MODE.get().cloned().unwrap_or_else(|| "rendu : pas encore decide".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La decision doit etre prise et NOMMEE : un mode inconnu dans le journal ne renseigne
    /// personne, et c'est ce journal qui jugera si le contournement a servi.
    #[test]
    fn la_decision_est_nommee() {
        decider();
        let mode = mode();
        assert!(mode.starts_with("rendu : "), "mode inattendu : {mode}");
        assert!(!mode.contains("pas encore decide"), "la decision n'a pas ete prise");
    }
}
