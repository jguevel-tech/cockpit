//! Le guetteur : la fenetre repond-elle, et l'ecran se met-il a jour ?
//!
//! **Pourquoi il existe.** Un gel ne laissait AUCUNE trace : l'utilisateur tuait l'application,
//! la relancait, et il ne restait rien a lire. Impossible de dire si la fenetre s'etait figee
//! parce que du code attendait un verrou, ou parce que le moteur de rendu avait cesse de
//! peindre. Deux causes, deux corrections opposees, aucune preuve.
//!
//! **Ce qu'il mesure.** La boucle principale doit lever la main chaque seconde, et la page doit
//! rendre compte de ses images. Trois pannes se distinguent alors, et elles ne se corrigent pas
//! au meme endroit : la boucle ne repond plus (notre code attend), la page ne peint plus (le
//! moteur de rendu), la page ne parle plus (son JavaScript est arrete).
//!
//! **AUCUNE HORLOGE ICI, ET C'EST DELIBERE.** La premiere version comptait en heure murale :
//! une mise en veille de 53 minutes lui a fait annoncer un gel de 3 180 secondes qui n'avait
//! jamais eu lieu (constate le 2026-08-31). On compte donc des TOURS et des REPONSES — pendant
//! une veille le fil ne tourne pas, donc rien ne s'accumule.
//!
//! **ET ON N'ECRIT QU'AU CHANGEMENT D'ETAT.** Se reperer sur le MESSAGE, qui portait un nombre
//! de secondes, a fait ecrire 733 lignes pour une poignee d'episodes : chaque seconde produisait
//! un message different, donc « nouveau ». Le repere est l'ETAT.
//!
//! Il ne corrige rien et ne tue rien : une application figee qui se fermerait d'elle-meme
//! emporterait le travail en cours sans rien expliquer.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

/// Cadence des demandes. Une par seconde : assez fin pour situer un gel, assez rare pour ne rien
/// couter a la boucle qu'on surveille.
const CADENCE: Duration = Duration::from_secs(1);

/// Tours sans reponse de la boucle principale avant de parler. En dessous, ce n'est pas un gel
/// mais une boucle occupee : un gros rendu, une ouverture de projet.
const TOURS_SANS_REPONSE: u32 = 5;

/// Tours sans compte rendu de la page avant de parler. Elle parle toutes les cinq secondes : le
/// seuil laisse passer trois periodes, sinon un simple retard passerait pour une panne.
const TOURS_SANS_RAPPORT: u32 = 15;

/// Combien de fois la boucle principale a leve la main.
static REPONSES: AtomicU64 = AtomicU64::new(0);

/// Combien de fois la page a rendu compte, et ce qu'elle a dit la derniere fois.
static RAPPORTS: AtomicU64 = AtomicU64::new(0);

/// A-t-elle peint au moins une image depuis son passage precedent ?
///
/// Un BOOLEEN et non un compte : la page ne demande plus qu'UNE image par periode. Compter les
/// images obligeait a en redemander soixante fois par seconde, ce qui ralentissait l'interface
/// et faisait sauter des lettres en cours de frappe (constate le 2026-08-31). La question posee
/// est « le moteur peint-il encore », pas « a quelle cadence ».
static A_PEINT: AtomicBool = AtomicBool::new(true);

/// La fenetre etait-elle visible ? **Une page cachee ne peint pas, et c'est NORMAL** : sans
/// cette information, passer sur une autre application accusait le moteur de rendu.
static VISIBLE: AtomicBool = AtomicBool::new(true);

/// Ce que le backend est en train de faire, pose par les chemins qui peuvent attendre longtemps.
/// Un gel nomme vaut dix hypotheses.
static EN_COURS: Mutex<Option<(&'static str, u64)>> = Mutex::new(None);

/// Marque une operation en cours. Le nom reapparait dans le journal si la fenetre se fige
/// pendant ce temps.
///
/// La marque se retire toute seule (`Drop`) : un chemin qui rend une erreur en plein milieu ne
/// doit pas laisser un nom colle la pour le reste de la session.
pub struct Marque;

pub fn marquer(quoi: &'static str) -> Marque {
    *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) =
        Some((quoi, REPONSES.load(Ordering::SeqCst)));
    Marque
}

impl Drop for Marque {
    fn drop(&mut self) {
        *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

fn ce_qui_tourne() -> String {
    match *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) {
        Some((quoi, depuis)) => {
            let tours = REPONSES.load(Ordering::SeqCst).saturating_sub(depuis);
            format!("{quoi}, commence il y a {tours} tour(s)")
        }
        None => "rien de marque cote backend".to_string(),
    }
}

/// La page rend compte : depuis son dernier passage elle a peint ou non, et la fenetre etait
/// visible ou non.
pub fn signe_de_la_page(a_peint: bool, visible: bool) {
    A_PEINT.store(a_peint, Ordering::SeqCst);
    VISIBLE.store(visible, Ordering::SeqCst);
    RAPPORTS.fetch_add(1, Ordering::SeqCst);
}

/// Les pannes que le guetteur sait nommer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panne {
    /// La boucle graphique ne repond plus : notre code attend quelque chose.
    BoucleFigee,
    /// La page parle, la fenetre est visible, et rien n'est peint : le moteur de rendu.
    RenduArrete,
    /// Plus aucun compte rendu de la page : son JavaScript est arrete.
    PageMuette,
}

/// Ce que les compteurs disent. Pur, donc verifiable sans boucle graphique ni minuteur.
///
/// L'ordre compte : une boucle figee explique tout le reste, on ne l'accuse pas deux fois.
pub fn diagnostiquer(
    tours_sans_reponse: u32,
    tours_sans_rapport: u32,
    visible: bool,
    a_peint: bool,
) -> Option<Panne> {
    if tours_sans_reponse >= TOURS_SANS_REPONSE {
        return Some(Panne::BoucleFigee);
    }
    if tours_sans_rapport >= TOURS_SANS_RAPPORT {
        return Some(Panne::PageMuette);
    }
    // Une fenetre cachee ne peint pas : ce n'est pas une panne. Et on ne juge le rendu que sur
    // un compte rendu FRAIS — un rapport en retard dirait n'importe quoi.
    if visible && !a_peint && tours_sans_rapport == 0 {
        return Some(Panne::RenduArrete);
    }
    None
}

fn phrase(panne: Panne) -> String {
    match panne {
        Panne::BoucleFigee => format!("la fenetre ne repond plus — {}", ce_qui_tourne()),
        Panne::RenduArrete => "la page ne peint AUCUNE image alors qu'elle est visible et \
                               qu'elle parle encore : c'est le moteur de rendu, pas notre code"
            .to_string(),
        Panne::PageMuette => "la page ne rend plus compte : son JavaScript est arrete".to_string(),
    }
}

/// Met le guetteur en route. A appeler une fois, au demarrage.
pub fn surveiller(app: AppHandle) {
    std::thread::spawn(move || {
        // UNE LIGNE AU DEMARRAGE, ET ELLE EST INDISPENSABLE. Sans elle, le silence du guetteur
        // veut dire deux choses opposees — « tout va bien » ou « le guetteur est casse » — et il
        // a fallu chercher sa phrase DANS le binaire monte pour trancher (2026-08-28). Elle dit
        // aussi quel mode de rendu a ete retenu : c'est ce journal qui jugera le contournement.
        journaliser(&app, &format!("guetteur en marche — {}", crate::rendu::mode()));

        let mut vues_reponses = REPONSES.load(Ordering::SeqCst);
        let mut vus_rapports = RAPPORTS.load(Ordering::SeqCst);
        let mut sans_reponse = 0u32;
        let mut sans_rapport = 0u32;
        let mut signalee: Option<Panne> = None;

        loop {
            std::thread::sleep(CADENCE);

            // La demande : si la boucle principale tourne, cette fermeture s'execute et le
            // compteur bouge. Si elle est figee, il ne bouge pas — c'est tout le signal.
            if app
                .run_on_main_thread(|| {
                    REPONSES.fetch_add(1, Ordering::SeqCst);
                })
                .is_err()
            {
                // La boucle n'existe plus : l'application se ferme.
                return;
            }

            let reponses = REPONSES.load(Ordering::SeqCst);
            sans_reponse = if reponses == vues_reponses { sans_reponse + 1 } else { 0 };
            vues_reponses = reponses;

            let rapports = RAPPORTS.load(Ordering::SeqCst);
            sans_rapport = if rapports == vus_rapports { sans_rapport + 1 } else { 0 };
            vus_rapports = rapports;

            let panne = diagnostiquer(
                sans_reponse,
                sans_rapport,
                VISIBLE.load(Ordering::SeqCst),
                A_PEINT.load(Ordering::SeqCst),
            );

            if panne != signalee {
                match panne {
                    Some(nouvelle) => journaliser(&app, &phrase(nouvelle)),
                    None => journaliser(&app, "tout est revenu a la normale"),
                }
                signalee = panne;
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
    fn une_boucle_occupee_quelques_tours_n_est_pas_un_gel() {
        assert_eq!(diagnostiquer(0, 0, true, true), None);
        assert_eq!(diagnostiquer(TOURS_SANS_REPONSE - 1, 0, true, true), None);
    }

    #[test]
    fn une_boucle_qui_ne_repond_plus_est_nommee() {
        assert_eq!(diagnostiquer(TOURS_SANS_REPONSE, 0, true, true), Some(Panne::BoucleFigee));
    }

    /// Une boucle figee explique aussi le silence de la page : un seul verdict, celui d'amont.
    #[test]
    fn la_boucle_figee_passe_devant_les_autres_pannes() {
        assert_eq!(
            diagnostiquer(TOURS_SANS_REPONSE, TOURS_SANS_RAPPORT, true, false),
            Some(Panne::BoucleFigee)
        );
    }

    #[test]
    fn une_page_qui_ne_peint_plus_accuse_le_moteur_de_rendu() {
        assert_eq!(diagnostiquer(0, 0, true, false), Some(Panne::RenduArrete));
    }

    /// **Le faux positif a ne pas reintroduire.** Passer sur une autre application arrete les
    /// images, et ce n'est pas une panne : la premiere version accusait le moteur de rendu
    /// chaque fois que la fenetre passait derriere une autre.
    #[test]
    fn une_fenetre_cachee_ne_peint_pas_et_ce_n_est_pas_une_panne() {
        assert_eq!(diagnostiquer(0, 0, false, false), None);
    }

    #[test]
    fn une_page_qui_se_tait_est_nommee_autrement() {
        assert_eq!(diagnostiquer(0, TOURS_SANS_RAPPORT, true, true), Some(Panne::PageMuette));
    }

    /// Un retard d'un tour ou deux n'est pas un silence : la page parle toutes les cinq
    /// secondes, le seuil laisse passer trois periodes.
    #[test]
    fn un_retard_de_la_page_n_est_pas_un_silence() {
        assert_eq!(diagnostiquer(0, 6, true, true), None);
    }

    #[test]
    fn une_marque_se_retire_toute_seule() {
        assert_eq!(ce_qui_tourne(), "rien de marque cote backend");
        {
            let _marque = marquer("lancement du service de terminaux");
            assert!(ce_qui_tourne().starts_with("lancement du service de terminaux, commence"));
        }
        // Sans le `Drop`, ce nom resterait colle pour le reste de la session et le prochain gel
        // accuserait une operation terminee depuis longtemps.
        assert_eq!(ce_qui_tourne(), "rien de marque cote backend");
    }
}
