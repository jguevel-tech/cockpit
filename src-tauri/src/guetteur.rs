//! Le guetteur de la boucle graphique.
//!
//! **Pourquoi il existe.** Un gel ne laissait AUCUNE trace : l'utilisateur tuait
//! l'application, la relancait, et il ne restait rien a lire. Impossible de dire si la
//! fenetre s'etait figee parce que du code attendait un verrou, ou parce que le moteur de
//! rendu avait cesse de peindre. Deux causes, deux corrections opposees, aucune preuve.
//!
//! **Ce qu'il fait.** Il demande a la boucle principale de lever la main une fois par
//! seconde. Quand elle ne la leve plus, il ecrit une ligne dans le journal local : depuis
//! combien de temps, et ce que le backend etait en train de faire. Il ne corrige rien — il
//! rend le prochain incident lisible.
//!
//! **Ce qu'il ne fait pas.** Il ne tue rien et ne relance rien. Une application figee qui se
//! ferme d'elle-meme emporterait le travail en cours sans rien expliquer.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

/// Cadence des demandes. Une par seconde : assez fin pour situer un gel, assez rare pour ne
/// rien couter a la boucle qu'on surveille.
const CADENCE: Duration = Duration::from_secs(1);

/// En dessous, ce n'est pas un gel mais une boucle occupee : un gros rendu, une ouverture de
/// projet. Au-dela, l'utilisateur voit une fenetre morte.
const SEUIL: Duration = Duration::from_secs(5);

/// Tant que le gel dure, on ne reecrit qu'a cet intervalle : sans ca, une fenetre figee
/// pendant dix minutes remplirait le journal qu'on veut lire.
const RAPPEL: Duration = Duration::from_secs(30);

/// Dernier signe de vie de la boucle principale, en millisecondes depuis l'epoque.
static DERNIER_SIGNE: AtomicI64 = AtomicI64::new(0);

/// Ce que le backend est en train de faire, pose par les chemins qui peuvent attendre
/// longtemps. Un gel nomme vaut dix hypotheses.
static EN_COURS: Mutex<Option<(&'static str, i64)>> = Mutex::new(None);

fn maintenant_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

/// Marque une operation en cours. Le nom reapparait dans le journal si la boucle graphique
/// se fige pendant ce temps.
///
/// La marque se retire toute seule (`Drop`) : un chemin qui rend une erreur en plein milieu
/// ne doit pas laisser un nom colle la pour le reste de la session.
pub struct Marque;

pub fn marquer(quoi: &'static str) -> Marque {
    *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) = Some((quoi, maintenant_ms()));
    Marque
}

impl Drop for Marque {
    fn drop(&mut self) {
        *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Ce que le backend faisait, pret a ecrire.
fn ce_qui_tourne() -> String {
    match *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) {
        Some((quoi, depuis)) => {
            format!("{quoi} depuis {} ms", (maintenant_ms() - depuis).max(0))
        }
        None => "rien de marque cote backend".to_string(),
    }
}

/// Faut-il ecrire une ligne ?
///
/// Separee du minuteur pour etre verifiable sans boucle graphique : la regle est ici, et les
/// essais du bas du fichier la tiennent.
pub fn doit_signaler(retard: Duration, deja_signale: Option<Duration>) -> bool {
    if retard < SEUIL {
        return false;
    }
    match deja_signale {
        None => true,
        Some(precedent) => retard >= precedent + RAPPEL,
    }
}

/// Met le guetteur en route. A appeler une fois, au demarrage.
pub fn surveiller(app: AppHandle) {
    DERNIER_SIGNE.store(maintenant_ms(), Ordering::SeqCst);

    std::thread::spawn(move || {
        let mut signale: Option<Duration> = None;
        loop {
            std::thread::sleep(CADENCE);

            // La demande : si la boucle principale tourne, cette fermeture s'execute tout
            // de suite. Si elle est figee, elle ne s'execute pas — et c'est le silence qui
            // nous renseigne.
            if app
                .run_on_main_thread(|| {
                    DERNIER_SIGNE.store(maintenant_ms(), Ordering::SeqCst);
                })
                .is_err()
            {
                // La boucle n'existe plus : l'application se ferme. Rien a signaler.
                return;
            }

            let retard = Duration::from_millis(
                (maintenant_ms() - DERNIER_SIGNE.load(Ordering::SeqCst)).max(0) as u64,
            );

            if doit_signaler(retard, signale) {
                journaliser(
                    &app,
                    &format!(
                        "la fenetre ne repond plus depuis {} s — {}",
                        retard.as_secs(),
                        ce_qui_tourne()
                    ),
                );
                signale = Some(retard);
            } else if retard < SEUIL {
                if let Some(precedent) = signale.take() {
                    journaliser(
                        &app,
                        &format!(
                            "la fenetre repond de nouveau (figee au moins {} s)",
                            precedent.as_secs()
                        ),
                    );
                }
            }
        }
    });
}

fn journaliser(app: &AppHandle, message: &str) {
    if let Ok(dir) = app.path().app_data_dir() {
        let horodatage = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        crate::report::append_log(
            &dir,
            &crate::report::format_log_line(&horodatage, "guetteur", message),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_boucle_occupee_quelques_secondes_n_est_pas_un_gel() {
        assert!(!doit_signaler(Duration::from_secs(2), None));
        assert!(!doit_signaler(SEUIL - Duration::from_millis(1), None));
    }

    #[test]
    fn le_premier_depassement_est_signale() {
        assert!(doit_signaler(SEUIL, None));
        assert!(doit_signaler(Duration::from_secs(30), None));
    }

    #[test]
    fn un_gel_qui_dure_ne_remplit_pas_le_journal() {
        let deja = Some(Duration::from_secs(6));
        // Une seconde plus tard : rien de neuf a dire.
        assert!(!doit_signaler(Duration::from_secs(7), deja));
        // Trente secondes plus tard : la ligne suivante est utile, le gel s'installe.
        assert!(doit_signaler(Duration::from_secs(36), deja));
    }

    #[test]
    fn une_marque_se_retire_toute_seule() {
        assert_eq!(ce_qui_tourne(), "rien de marque cote backend");
        {
            let _marque = marquer("lancement du service de terminaux");
            assert!(ce_qui_tourne().starts_with("lancement du service de terminaux depuis"));
        }
        // Sans le `Drop`, ce nom resterait colle pour le reste de la session et le
        // prochain gel accuserait une operation terminee depuis longtemps.
        assert_eq!(ce_qui_tourne(), "rien de marque cote backend");
    }
}
