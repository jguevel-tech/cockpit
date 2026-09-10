#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Le service de terminaux tourne dans CE binaire, lance avec `--service-terminaux`.
    // Le test doit venir avant tout le reste : ce processus n'ouvre pas de fenetre.
    if cockpit_lib::service_terminaux_si_demande() {
        return;
    }
    // Le pont sert les memes commandes a un hote qui n'est pas Tauri. Comme le service, il
    // n'ouvre aucune fenetre : le test vient donc avant `run()`.
    if cockpit_lib::pont_si_demande() {
        return;
    }
    #[cfg(feature = "interface-tauri")]
    cockpit_lib::run();

    // **SANS TAURI, CE BINAIRE N'OUVRE AUCUNE FENETRE**, et c'est le but : il ne sert que le
    // pont et le service de terminaux. Lance sans mode, il ne doit pas rendre la main en
    // silence — on croirait a un demarrage reussi.
    #[cfg(not(feature = "interface-tauri"))]
    {
        eprintln!(
            "cockpit : construit sans interface graphique. Modes disponibles :\n\
             \x20 --pont                        servir les commandes sur l'entree standard\n\
             \x20 --service-terminaux <socket>  tenir les terminaux"
        );
        std::process::exit(2);
    }
}
