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

/// Dernier compte rendu de la PAGE : quand, et combien d'images elle a dessinees depuis le
/// precedent.
///
/// **Pourquoi la page doit parler separement.** Le 2026-08-28, un gel a eu lieu alors que la
/// boucle principale repondait normalement : le guetteur n'avait donc rien a dire, et son
/// silence ne prouvait rien. Or « la boucle tourne » et « l'ecran se met a jour » sont deux
/// choses differentes — un moteur de rendu qui ne peint plus laisse une fenetre morte sous une
/// application en parfaite sante. Les minuteurs JavaScript continuent de tomber dans ce cas, la
/// demande d'image NON : c'est ce qui les separe.
static DERNIER_SIGNE_PAGE: AtomicI64 = AtomicI64::new(0);
static IMAGES_PAGE: AtomicI64 = AtomicI64::new(0);

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

/// La page rend compte : depuis son dernier passage, elle a dessine `images` images.
///
/// `images` a zero avec un compte rendu qui arrive quand meme veut dire quelque chose de
/// precis : le JavaScript tourne, mais rien n'est peint.
pub fn signe_de_la_page(images: u32) {
    DERNIER_SIGNE_PAGE.store(maintenant_ms(), Ordering::SeqCst);
    IMAGES_PAGE.store(images as i64, Ordering::SeqCst);
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

/// La page a-t-elle cesse de peindre, alors qu'elle parle encore ?
///
/// Trois etats se distinguent, et c'est tout l'interet : la boucle principale ne repond plus
/// (notre code attend quelque chose), la page parle mais ne peint plus (le moteur de rendu),
/// ou la page ne parle plus du tout (son JavaScript est arrete).
fn etat_de_la_page() -> Option<String> {
    let dernier = DERNIER_SIGNE_PAGE.load(Ordering::SeqCst);
    // Aucun compte rendu encore : le demarrage, rien a dire.
    if dernier == 0 {
        return None;
    }
    let depuis = Duration::from_millis((maintenant_ms() - dernier).max(0) as u64);
    if depuis >= SEUIL {
        return Some(format!("la page ne parle plus depuis {} s", depuis.as_secs()));
    }
    if IMAGES_PAGE.load(Ordering::SeqCst) == 0 {
        return Some(
            "la page parle mais ne peint AUCUNE image : c'est le moteur de rendu, pas notre code"
                .to_string(),
        );
    }
    None
}

/// Met le guetteur en route. A appeler une fois, au demarrage.
pub fn surveiller(app: AppHandle) {
    DERNIER_SIGNE.store(maintenant_ms(), Ordering::SeqCst);

    std::thread::spawn(move || {
        // UNE LIGNE AU DEMARRAGE, ET ELLE EST INDISPENSABLE. Sans elle, le silence du
        // guetteur veut dire deux choses opposees — « tout va bien » ou « le guetteur est
        // casse » — et on ne peut pas les distinguer. C'est arrive : le 2026-08-28, un gel
        // n'a laisse aucune ligne, et il a fallu chercher la phrase du guetteur DANS le
        // binaire pour savoir s'il tournait.
        journaliser(&app, "guetteur en marche");

        let mut signale: Option<Duration> = None;
        let mut page_signalee: Option<String> = None;
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

                // La boucle va bien : c'est le moment ou la question « et l'ecran ? » a un
                // sens. On ne la pose pas quand la boucle est deja en cause, sinon deux
                // lignes accusent la meme panne.
                match etat_de_la_page() {
                    Some(quoi) => {
                        if page_signalee.as_deref() != Some(quoi.as_str()) {
                            journaliser(&app, &quoi);
                            page_signalee = Some(quoi);
                        }
                    }
                    None => {
                        if page_signalee.take().is_some() {
                            journaliser(&app, "la page se peint de nouveau");
                        }
                    }
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

    /// Les trois etats de la page, dans un seul essai : ces compteurs sont globaux, et deux
    /// essais qui y touchent en parallele se marcheraient dessus.
    #[test]
    fn les_trois_etats_de_la_page_se_distinguent() {
        // Au demarrage, la page n'a rien dit encore : on n'accuse personne.
        assert_eq!(etat_de_la_page(), None, "aucun compte rendu ne doit rien accuser");

        // Elle peint : rien a signaler.
        signe_de_la_page(42);
        assert_eq!(etat_de_la_page(), None);

        // Elle parle et ne peint plus : c'est le moteur de rendu.
        signe_de_la_page(0);
        let verdict = etat_de_la_page().expect("zero image doit se voir");
        assert!(verdict.contains("moteur de rendu"), "verdict inattendu : {verdict}");

        // Elle ne parle plus du tout : autre panne, autre message. On vieillit son dernier
        // passage au-dela du seuil plutot que d'attendre reellement.
        signe_de_la_page(42);
        DERNIER_SIGNE_PAGE.store(
            maintenant_ms() - (SEUIL.as_millis() as i64) - 1_000,
            Ordering::SeqCst,
        );
        let verdict = etat_de_la_page().expect("un silence de la page doit se voir");
        assert!(verdict.contains("ne parle plus"), "verdict inattendu : {verdict}");

        // Et on remet les compteurs comme on les a trouves.
        DERNIER_SIGNE_PAGE.store(0, Ordering::SeqCst);
        IMAGES_PAGE.store(0, Ordering::SeqCst);
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
