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
//! **MAIS LE CONTOURNEMENT A UN PRIX, ET IL A ETE PAYE.** Sans le chemin DMA-BUF, la page est
//! composee par le processeur. Pose automatiquement en 0.54.2, il a rendu l'interface plus lente
//! et fait sauter des lettres en cours de frappe — constate le jour meme. Un gel occasionnel se
//! recupere en relancant, une frappe qui saute rend le logiciel inutilisable a la minute : le
//! reglage est donc revenu entre les mains de l'utilisateur.
//!
//! Ce module ne decide plus rien : il CONSTATE et l'ecrit dans le journal, avec la marche a
//! suivre quand le pilote fautif est present.

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
///
/// **ON NE COUPE PLUS RIEN TOUT SEUL, ET C'EST UN RETOUR EN ARRIERE ASSUME.** La version 0.54.2
/// desactivait le chemin DMA-BUF des qu'elle voyait le pilote NVIDIA. Le contournement est reel
/// et documente en amont, mais son PRIX ne l'etait pas : sans ce chemin, la page est composee par
/// le processeur, et le mainteneur a constate le jour meme une interface plus lente et des
/// lettres qui s'effacaient en cours de frappe. Un gel occasionnel se recupere en relancant ;
/// une frappe qui saute rend le logiciel inutilisable a la minute.
///
/// La variable reste donc entre les mains de qui la pose, et le mode retenu est ECRIT au
/// demarrage : c'est ce qui permettra de comparer les deux, journal en main, au lieu de choisir
/// pour tout le monde sans mesure.
#[cfg(target_os = "linux")]
pub fn decider() {
    let mode = match std::env::var(VARIABLE) {
        Ok(pose) => format!("rendu : {VARIABLE}={pose} (pose par l'utilisateur)"),
        Err(_) if pilote_nvidia_proprietaire() => {
            "rendu : chemin normal, pilote NVIDIA proprietaire present — si la fenetre cesse de \
             se redessiner, lancer avec WEBKIT_DISABLE_DMABUF_RENDERER=1"
                .to_string()
        }
        Err(_) => "rendu : chemin normal".to_string(),
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
