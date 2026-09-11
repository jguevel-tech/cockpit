//! Ouvrir une adresse dans le navigateur de la personne.
//!
//! **POURQUOI CE MODULE EXISTE.** L'ouverture passait par un plugin de Tauri, qui tirait
//! `gtk` et `atk` : le binaire reclamait GTK A LA COMPILATION alors qu'il n'en a aucun
//! besoin, et la CI Linux tombait dessus. Le lanceur du systeme suffit.

/// Ouvre l'adresse. **C'est le navigateur de la personne qui doit s'ouvrir**, celui ou elle
/// est deja connectee, jamais une vue interne.
pub fn adresse(url: &str) -> Result<(), String> {
    // Le lanceur de chaque systeme. `.sans_console()` est obligatoire : sous Windows une
    // application graphique n'a pas de console, et chaque programme lance en ouvrirait une
    // — la regle vaut pour TOUTE commande externe du projet, sans exception.
    use crate::commande::SansConsole;
    let (programme, avant) = if cfg!(target_os = "macos") {
        ("open", None)
    } else if cfg!(target_os = "windows") {
        // `start` est une commande interne de cmd, pas un programme ; le premier argument y
        // est le TITRE de la fenetre, d'ou la chaine vide.
        ("cmd", Some(vec!["/C", "start", ""]))
    } else {
        ("xdg-open", None)
    };
    let mut commande = std::process::Command::new(programme);
    commande.sans_console();
    if let Some(arguments) = avant {
        commande.args(arguments);
    }
    commande
        .arg(url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{programme} : {e}"))
}
