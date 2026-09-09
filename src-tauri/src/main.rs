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
    cockpit_lib::run()
}
