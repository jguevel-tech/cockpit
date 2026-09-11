#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Le service de terminaux tourne dans CE binaire, lance avec `--service-terminaux`.
    if cockpit_lib::service_terminaux_si_demande() {
        return;
    }
    // Le pont sert les commandes a la coquille, sur l'entree standard.
    if cockpit_lib::pont_si_demande() {
        return;
    }

    // **CE BINAIRE N'OUVRE AUCUNE FENETRE**, et c'est le but : l'affichage appartient a la
    // coquille Electron. Lance sans mode, il ne doit pas rendre la main en silence — on
    // croirait a un demarrage reussi.
    eprintln!(
        "cockpit : ce binaire ne sert que la coquille. Modes disponibles :\n\
         \x20 --pont                        servir les commandes sur l'entree standard\n\
         \x20 --service-terminaux <socket>  tenir les terminaux"
    );
    std::process::exit(2);
}
