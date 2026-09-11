//! Le guetteur : la fenetre repond-elle, l'ecran se met-il a jour, et QUE FAIT-ON quand non ?
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
//! **CE QU'IL REPARE, depuis le 2026-09-09.** Observer sans agir laissait l'utilisateur seul
//! devant une fenetre morte : le journal du 2026-09-08 montre 45 episodes en une journee, dont
//! un gel de plus d'une demi-heure termine par un kill. Les paliers, dans l'ordre :
//!   1. la page ne peint plus (ou ne parle plus) ET l'utilisateur est devant → la vue est
//!      RECHARGEE. Les terminaux ne perdent rien : le service les tient, la vue s'y rebranche ;
//!   2. les rechargements ne suffisent pas → l'application se RELANCE elle-meme, comme
//!      l'utilisateur le ferait a la main, en mieux : les shells survivent dans le service ;
//!   3. la boucle principale ne repond plus du tout pendant une minute → relance aussi, car
//!      rien d'autre dans le processus ne peut plus la debloquer ;
//!   4. les episodes se repetent → l'application PROPPOSE le mode secours du rendu
//!      (`rendu`), au moment ou l'affichage va de nouveau, donc quand un dialogue est lisible.
//!
//! **LA PRESENCE DE L'UTILISATEUR EST LA CONDITION DE TOUT, SAUF DU PALIER 3.** La page dit
//! toutes les cinq secondes si le clavier ou la souris a servi recemment. Sans cette porte, le
//! guetteur agirait sur une fenetre que personne ne regarde : la nuit du 2026-09-03 au 09-08,
//! les trois quarts des episodes ont eu lieu entre 21 h et 7 h — ecran eteint ou verrouille,
//! fenetre « visible » pour la page, aucune image peinte, et personne devant. Recharger une vue
//! dans le vide ne gene personne mais ne prouve rien non plus ; se relancer toute la nuit, si.
//! Le palier 3 s'en passe : une boucle morte une minute est morte, et justement la page ne
//! peut plus rien nous dire — son IPC passe par la boucle graphique.
//!
//! **AUCUNE HORLOGE ICI, ET C'EST DELIBERE.** La premiere version comptait en heure murale :
//! une mise en veille de 53 minutes lui a fait annoncer un gel de 3 180 secondes qui n'avait
//! jamais eu lieu (constate le 2026-08-31). On compte donc des TOURS et des REPONSES — pendant
//! une veille le fil ne tourne pas, donc rien ne s'accumule. Les delais des paliers sont dans
//! la meme unite : une nuit de veille ne rapproche pas un relancement.
//!
//! **ET ON N'ECRIT QU'AU CHANGEMENT D'ETAT.** Se reperer sur le MESSAGE, qui portait un nombre
//! de secondes, a fait ecrire 733 lignes pour une poignee d'episodes : chaque seconde produisait
//! un message different, donc « nouveau ». Le repere est l'ETAT.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// **CE QUI SUIT EST COUPE EN DEUX.** Ce que le guetteur MESURE et DECIDE ne connait ni
/// fenetre ni hote : ce sont des compteurs et des fonctions pures, et c'est ce qui les rend
/// verifiables. Ce qu'il FAIT (recharger la vue, relancer, ecrire le journal) passe par
/// l'hote — ici Tauri. Une autre coquille rebranchera ces gestes-la, pas les decisions.
#[cfg(feature = "interface-tauri")]
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

/// Comptes rendus consecutifs sans une seule image avant d'accuser le moteur de rendu.
///
/// **UN SEUL NE PROUVE RIEN.** Une fenetre recouverte par une autre garde
/// `visibilityState = "visible"` et cesse pourtant de produire des images : juger sur un seul
/// compte rendu accusait donc le moteur de rendu des qu'on travaillait ailleurs. Le journal du
/// 2026-08-31 en est plein — il alternait « ne peint plus » et « revenu a la normale » toutes
/// les cinq secondes.
const RAPPORTS_SANS_IMAGE: u32 = 3;

/// Tours de panne confirmee avant de recharger la vue.
///
/// **POURQUOI 5 ET PAS 25.** A 25 tours la reparation tombait 40 secondes apres le debut du
/// gel — le 2026-09-09 a 14 h, l'utilisateur a tue l'application a la 12e seconde de panne
/// signalee, sans savoir qu'un rechargement arrivait. Or un gel de 20 a 45 secondes qui
/// « revient tout seul » (le motif du 2026-09-08) est vecu exactement comme un gel qu'on tue.
/// La marge ne protegeait donc rien : cinq tours suffisent a ecarter un hoquet de rendu, et
/// la page en doute rapporte chaque seconde, donc la confirmation elle-meme prend ~3 s.
const TOURS_AVANT_RECHARGEMENT: u32 = 5;

/// Tours entre deux rechargements. Un affichage qui gèle toutes les minutes (le motif du
/// 2026-09-08, 15 h 51 a 16 h 11) ne doit pas transformer l'application en boucle de
/// rechargement permanent : passe ce delai, le prochain episode attend son tour.
const TOURS_ENTRE_RECHARGEMENTS: u64 = 60;

/// Rechargements dans la fenetre d'observation avant de conclure que recharger ne suffit pas.
const RECHARGEMENTS_MAX: u32 = 3;

/// Tours de panne de rendu, rechargements epuises, avant de relancer toute l'application.
const TOURS_AVANT_RELANCER_RENDU: u32 = 120;

/// Tours sans reponse de la boucle principale avant de relancer l'application. Une minute
/// reelle : en dessous, un gros travail (ouverture de projet, construction) peut legitiment
/// tenir la boucle. Au-dela, plus rien dans le processus ne peut la debloquer, et
/// l'utilisateur devrait sinon tuer l'application a la main.
const TOURS_AVANT_RELANCER_BOUCLE: u32 = 60;

/// Tours depuis la derniere entree utilisateur au-dela desquels personne n'est devant la
/// fenetre. La page rapporte les entrees des deux dernieres minutes, toutes les cinq
/// secondes : cinq tours de marge couvrent un rapport manque.
const TOURS_SANS_PRESENCE: u64 = 300;

/// Fenetre d'observation des episodes et des rechargements, en tours (donc en secondes).
const FENETRE_OBSERVATION: u64 = 1800;

/// Episodes de rendu confirmes dans la fenetre d'observation a partir desquels le mode
/// secours est propose. En dessous, c'est un incident ; au-dela, c'est une configuration
/// qui ne sait pas peindre de facon fiable.
const EPISODES_AVANT_PROPOSITION: usize = 3;

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

/// La fenetre avait-elle le focus ? **Une fenetre RECOUVERTE reste « visible » pour la page
/// et cesse de produire des images** : c'etait le faux positif du 2026-08-31, et `visible`
/// seul ne l'ecartait pas. Le focus, si : on regarde ailleurs, elle ne l'a plus.
static CONCENTRE: AtomicBool = AtomicBool::new(true);

/// Le tour ou la page a signale une entree utilisateur pour la derniere fois. Sert de
/// presence : on ne repare que ce que quelqu'un regarde (voir l'en-tete).
static DERNIERE_ENTREE: AtomicU64 = AtomicU64::new(0);

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

#[cfg(feature = "interface-tauri")]
fn ce_qui_tourne() -> String {
    match *EN_COURS.lock().unwrap_or_else(|e| e.into_inner()) {
        Some((quoi, depuis)) => {
            let tours = REPONSES.load(Ordering::SeqCst).saturating_sub(depuis);
            format!("{quoi}, commence il y a {tours} tour(s)")
        }
        None => "rien de marque cote backend".to_string(),
    }
}

/// La page rend compte : depuis son dernier passage elle a peint ou non, la fenetre etait
/// visible et concentree ou non, et le clavier ou la souris a servi recemment ou non.
pub fn signe_de_la_page(a_peint: bool, visible: bool, concentre: bool, entree_recente: bool) {
    A_PEINT.store(a_peint, Ordering::SeqCst);
    VISIBLE.store(visible, Ordering::SeqCst);
    CONCENTRE.store(concentre, Ordering::SeqCst);
    if entree_recente {
        DERNIERE_ENTREE.store(REPONSES.load(Ordering::SeqCst), Ordering::SeqCst);
    }
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
    rapports_sans_image: u32,
) -> Option<Panne> {
    if tours_sans_reponse >= TOURS_SANS_REPONSE {
        return Some(Panne::BoucleFigee);
    }
    if tours_sans_rapport >= TOURS_SANS_RAPPORT {
        return Some(Panne::PageMuette);
    }
    if rapports_sans_image >= RAPPORTS_SANS_IMAGE {
        return Some(Panne::RenduArrete);
    }
    None
}

/// Met a jour le compte des rapports sans image. Pur.
///
/// Une fenetre cachee OU RECOUVERTE ne peint pas et ce n'est PAS une panne : elle remet le
/// compte a zero. Le focus est ce qui distingue « recouverte » de « gelee » : une fenetre
/// sous une autre garde `visible` et perd le focus.
pub fn compter_sans_image(
    precedent: u32,
    rapport_frais: bool,
    visible: bool,
    concentre: bool,
    a_peint: bool,
) -> u32 {
    if !rapport_frais {
        // Rien de neuf : on garde ce qu'on savait, sinon le verdict oscille a chaque tour.
        return precedent;
    }
    if !visible || !concentre || a_peint {
        return 0;
    }
    precedent + 1
}

/// Ce que le guetteur decide de faire face a une panne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Rien : observer, et ecrire au changement d'etat seulement.
    Rien,
    /// Recharger la vue. La boucle graphique repond dans ce cas, donc la commande passe.
    Recharger,
    /// Relancer toute l'application. Les shells survivent : le service de terminaux est un
    /// processus separe.
    Relancer,
}

/// Tout ce que la decision de reparer prend en compte. Pur, donc testable sans fenetre,
/// sans minuteur et sans tuer personne — meme decoupe que `diagnostiquer`.
#[derive(Debug, Clone, Copy)]
pub struct Etat {
    pub panne: Option<Panne>,
    /// Tours consecutifs avec cette panne.
    pub tours_en_panne: u32,
    /// Tours depuis la derniere entree utilisateur rapportee par la page.
    pub tours_depuis_entree: u64,
    /// Rechargements deja faits dans la fenetre d'observation.
    pub rechargements_fenetre: u32,
    /// Tours depuis le dernier rechargement. `u64::MAX` s'il n'y en a pas eu.
    pub tours_depuis_rechargement: u64,
}

/// La decision de reparation. **PURE : c'est elle que les essais jugent.**
///
/// Le palier de la boucle figee se passe de la presence : la page ne peut justement plus la
/// rapporter (son IPC passe par la boucle graphique), et une boucle morte une minute ne
/// revient pas.
pub fn prochaine_action(etat: &Etat) -> Action {
    let Some(panne) = etat.panne else {
        return Action::Rien;
    };
    match panne {
        Panne::BoucleFigee => {
            if etat.tours_en_panne >= TOURS_AVANT_RELANCER_BOUCLE {
                Action::Relancer
            } else {
                Action::Rien
            }
        }
        Panne::RenduArrete | Panne::PageMuette => {
            // Personne devant la fenetre : on laisse, le journal suffit. C'est la porte qui
            // empeche d'agir sur les episodes de la nuit (ecran eteint, fenetre « visible »).
            if etat.tours_depuis_entree > TOURS_SANS_PRESENCE {
                return Action::Rien;
            }
            if etat.rechargements_fenetre >= RECHARGEMENTS_MAX {
                // Recharger ne suffit pas : passer au processus entier, mais seulement si la
                // panne dure — elle peut encore se recuperer seule.
                return if etat.tours_en_panne >= TOURS_AVANT_RELANCER_RENDU {
                    Action::Relancer
                } else {
                    Action::Rien
                };
            }
            if etat.tours_en_panne >= TOURS_AVANT_RECHARGEMENT
                && etat.tours_depuis_rechargement >= TOURS_ENTRE_RECHARGEMENTS
            {
                Action::Recharger
            } else {
                Action::Rien
            }
        }
    }
}

/// Faut-il proposer le mode secours ? Pur. Au retour a la normale, pas pendant : un dialogue
/// ne se lit pas sur un ecran qui ne peint plus.
pub fn faut_il_proposer_le_secours(
    precedente: Option<Panne>,
    episodes_dans_la_fenetre: usize,
    deja_propose: bool,
) -> bool {
    !deja_propose
        && episodes_dans_la_fenetre >= EPISODES_AVANT_PROPOSITION
        && matches!(
            precedente,
            Some(Panne::RenduArrete) | Some(Panne::PageMuette)
        )
}

/// Retire d'une file les evenements plus vieux que la fenetre d'observation. Pur.
pub fn tailler_la_fenetre(file: &mut VecDeque<u64>, maintenant: u64) {
    while file.front().is_some_and(|t| maintenant.saturating_sub(*t) > FENETRE_OBSERVATION) {
        file.pop_front();
    }
}

#[cfg(feature = "interface-tauri")]
fn phrase(panne: Panne) -> String {
    match panne {
        Panne::BoucleFigee => format!("la fenetre ne repond plus — {}", ce_qui_tourne()),
        Panne::RenduArrete => "la page ne peint AUCUNE image alors qu'elle est visible, \
                               concentree et qu'elle parle encore : c'est le moteur de rendu, \
                               pas notre code"
            .to_string(),
        Panne::PageMuette => "la page ne rend plus compte : son JavaScript est arrete".to_string(),
    }
}

/// Met le guetteur en route. A appeler une fois, au demarrage.
#[cfg(feature = "interface-tauri")]
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
        let mut sans_image = 0u32;
        let mut signalee: Option<Panne> = None;
        let mut tours_en_panne = 0u32;
        // Tours (donc secondes) ou chaque episode et chaque rechargement a commence.
        let mut episodes: VecDeque<u64> = VecDeque::new();
        let mut rechargements: VecDeque<u64> = VecDeque::new();
        let mut secours_propose = false;

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

            sans_image = compter_sans_image(
                sans_image,
                sans_rapport == 0,
                VISIBLE.load(Ordering::SeqCst),
                CONCENTRE.load(Ordering::SeqCst),
                A_PEINT.load(Ordering::SeqCst),
            );

            let panne = diagnostiquer(sans_reponse, sans_rapport, sans_image);

            tailler_la_fenetre(&mut episodes, reponses);
            tailler_la_fenetre(&mut rechargements, reponses);
            let duree_episode = tours_en_panne;
            tours_en_panne = if panne.is_some() { tours_en_panne + 1 } else { 0 };

            if panne != signalee {
                match panne {
                    Some(nouvelle) => {
                        journaliser(&app, &phrase(nouvelle));
                        episodes.push_back(reponses);
                    }
                    None => {
                        // La DUREE est la mesure qui manquait : sans elle, « revenu a la
                        // normale » ne dit pas si le gel a dure 3 secondes ou 30 minutes,
                        // et on ne peut pas juger si les correctifs raccourcissent quoi
                        // que ce soit. Un tour = une seconde (la boucle ne dort pas
                        // pendant une veille, donc une veille ne gonfle rien).
                        journaliser(
                            &app,
                            &format!(
                                "tout est revenu a la normale — le gel avait dure {duree_episode} s"
                            ),
                        );
                        if faut_il_proposer_le_secours(signalee, episodes.len(), secours_propose) {
                            secours_propose = true;
                            journaliser(
                                &app,
                                &format!(
                                    "affichage gele {} fois en {} minutes : proposition du mode \
                                     secours de rendu",
                                    episodes.len(),
                                    FENETRE_OBSERVATION / 60
                                ),
                            );
                            proposer_mode_secours(&app);
                        }
                    }
                }
                signalee = panne;
            }

            let etat = Etat {
                panne,
                tours_en_panne,
                tours_depuis_entree: reponses
                    .saturating_sub(DERNIERE_ENTREE.load(Ordering::SeqCst)),
                rechargements_fenetre: rechargements.len() as u32,
                tours_depuis_rechargement: rechargements
                    .back()
                    .map(|t| reponses.saturating_sub(*t))
                    .unwrap_or(u64::MAX),
            };
            match prochaine_action(&etat) {
                Action::Rien => {}
                Action::Recharger => {
                    rechargements.push_back(reponses);
                    journaliser(
                        &app,
                        &format!(
                            "l'affichage est gele depuis {tours_en_panne} s et l'utilisateur \
                             est devant : je recharge la vue"
                        ),
                    );
                    marquer_vue_rechargee(&app);
                    recharger_la_vue(&app);
                }
                Action::Relancer => {
                    let cause = match panne {
                        Some(Panne::BoucleFigee) => "la fenetre ne repond plus depuis une minute",
                        _ => "recharger la vue n'a pas suffi",
                    };
                    journaliser(
                        &app,
                        &format!(
                            "je relance l'application : {cause} (gel de {tours_en_panne} s). \
                             Les terminaux survivent dans le service"
                        ),
                    );
                    // En cas d'echec la fonction a deja journalise : le gel continue,
                    // l'utilisateur garde la main (kill), rien de pire qu'avant.
                    let _ = relancer_l_application(&app);
                }
            }
        }
    });
}

/// Recharge la vue web. Passe par la boucle principale : dans les pannes ou cette action est
/// decidee (`RenduArrete`, `PageMuette`), elle repond encore — c'est justement ce qui les
/// distingue de `BoucleFigee`.
#[cfg(feature = "interface-tauri")]
fn recharger_la_vue(app: &AppHandle) {
    let app_pour_closure = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(fenetre) = app_pour_closure.get_webview_window("main") {
            if let Err(e) = fenetre.reload() {
                journaliser(&app_pour_closure, &format!("rechargement de la vue refuse : {e}"));
            }
        }
    });
}

/// Laisse une trace en base : au prochain montage, la page dira POURQUOI elle vient de se
/// remonter. Sans ca, l'utilisateur voit son interface clignoter et se reconstruire sans
/// explication — et un geste invisible est un geste qui fait peur.
#[cfg(feature = "interface-tauri")]
fn marquer_vue_rechargee(app: &AppHandle) {
    if let Some(etat) = app.try_state::<crate::AppState>() {
        if let Err(e) = etat.db.set_setting("gel_vue_rechargee", "1") {
            journaliser(app, &format!("trace du rechargement impossible a ecrire : {e}"));
        }
    }
}

/// Le dialogue de proposition ne se lit QUE quand l'affichage va de nouveau : l'evenement
/// est donc emis au retour a la normale, jamais pendant la panne.
#[cfg(feature = "interface-tauri")]
fn proposer_mode_secours(app: &AppHandle) {
    use tauri::Emitter;
    if let Err(e) = app.emit("guetteur-proposition-secours", ()) {
        journaliser(app, &format!("proposition du mode secours non transmise : {e}"));
    }
}

/// Relance l'application, puis quitte ce processus.
///
/// **Le binaire est choisi comme pour le service de terminaux** : `$APPIMAGE` d'abord, car le
/// montage `/tmp/.mount_*` disparait quand ce processus se termine. Sans console sous
/// Windows, comme tout lancement.
///
/// **LE NOM « SINGLE INSTANCE » EST LIBERE AVANT DE LANCER.** L'application empeche un
/// second exemplaire de demarrer : tant que ce processus vit, il tient le nom qui le
/// prouve, et une instance lancee pendant ce temps se tait et se tue. Sans cette
/// liberation, la nouvelle instance arrivait trop tot et disparaissait sans laisser une
/// ligne — constate au banc le 2026-09-09.
///
/// **Si le lancement echoue, on NE QUITTE PAS** : transformer un gel en disparition serait
/// aggraver la panne. La fenetre figee reste, et le journal dit pourquoi rien n'est venu.
#[cfg(feature = "interface-tauri")]
pub(crate) fn relancer_l_application(app: &AppHandle) -> Result<(), String> {
    tauri_plugin_single_instance::destroy(app);
    let binaire = crate::terminal::service::lancement::binaire_de_l_application().map_err(|e| {
        journaliser(app, &format!("relance impossible, executable introuvable : {e}"));
        e
    })?;
    use crate::commande::SansConsole;
    let mut commande = std::process::Command::new(binaire);
    commande.sans_console();
    commande.stdin(std::process::Stdio::null());
    commande.stdout(std::process::Stdio::null());
    commande.stderr(std::process::Stdio::null());
    match commande.spawn() {
        Ok(_) => {
            journaliser(app, "la nouvelle instance est lancee, celle-ci s'arrete");
            // Les photos de terminaux ne seront pas reprises (la boucle est morte ou
            // l'affichage gele) : la restauration jouera la derniere photo, comme apres
            // une extinction. C'est le comportement documente, pas une perte nouvelle.
            std::process::exit(0);
        }
        Err(e) => {
            let message = format!("relance impossible, la nouvelle instance n'a pas demarre : {e}");
            journaliser(app, &message);
            Err(message)
        }
    }
}

#[cfg(feature = "interface-tauri")]
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
        assert_eq!(diagnostiquer(0, 0, 0), None);
        assert_eq!(diagnostiquer(TOURS_SANS_REPONSE - 1, 0, 0), None);
    }

    #[test]
    fn une_boucle_qui_ne_repond_plus_est_nommee() {
        assert_eq!(diagnostiquer(TOURS_SANS_REPONSE, 0, 0), Some(Panne::BoucleFigee));
    }

    /// Une boucle figee explique aussi le silence de la page : un seul verdict, celui d'amont.
    #[test]
    fn la_boucle_figee_passe_devant_les_autres_pannes() {
        assert_eq!(
            diagnostiquer(TOURS_SANS_REPONSE, TOURS_SANS_RAPPORT, RAPPORTS_SANS_IMAGE),
            Some(Panne::BoucleFigee)
        );
    }

    #[test]
    fn une_page_qui_se_tait_est_nommee_autrement() {
        assert_eq!(diagnostiquer(0, TOURS_SANS_RAPPORT, 0), Some(Panne::PageMuette));
    }

    /// Un retard d'un tour ou deux n'est pas un silence : la page parle toutes les cinq
    /// secondes, le seuil laisse passer trois periodes.
    #[test]
    fn un_retard_de_la_page_n_est_pas_un_silence() {
        assert_eq!(diagnostiquer(0, 6, 0), None);
    }

    #[test]
    fn le_moteur_de_rendu_n_est_accuse_qu_apres_plusieurs_rapports() {
        assert_eq!(diagnostiquer(0, 0, RAPPORTS_SANS_IMAGE - 1), None);
        assert_eq!(diagnostiquer(0, 0, RAPPORTS_SANS_IMAGE), Some(Panne::RenduArrete));
    }

    /// **LES TROIS FAUX POSITIFS A NE PAS REINTRODUIRE.** Une fenetre RECOUVERTE cesse de
    /// produire des images tout en restant « visible » : c'est le focus qui la disculpe, pas
    /// `visible`, et sans lui le journal du 2026-08-31 s'est rempli d'alternances « ne peint
    /// plus / revenu a la normale ». Une fenetre cachee non plus n'est pas une panne. Et un
    /// tour sans compte rendu frais ne change RIEN — sinon le verdict oscille a chaque
    /// seconde.
    #[test]
    fn le_compte_des_rapports_sans_image_ne_se_laisse_pas_tromper() {
        // Un rapport frais sans image, fenetre visible et concentree : ca compte.
        assert_eq!(compter_sans_image(0, true, true, true, false), 1);
        assert_eq!(compter_sans_image(1, true, true, true, false), 2);
        // La page peint de nouveau : on repart de zero.
        assert_eq!(compter_sans_image(2, true, true, true, true), 0);
        // Fenetre cachee : ce n'est pas une panne.
        assert_eq!(compter_sans_image(2, true, false, true, false), 0);
        // Fenetre visible mais RECOUVERTE (une autre a le focus) : pas une panne non plus.
        // Retirer `!concentre` de `compter_sans_image` DOIT faire tomber cette ligne.
        assert_eq!(compter_sans_image(2, true, true, false, false), 0);
        // Aucun rapport neuf : on garde ce qu'on savait, sans osciller.
        assert_eq!(compter_sans_image(2, false, true, true, false), 2);
        assert_eq!(compter_sans_image(0, false, true, true, false), 0);
    }

    /// **CET ESSAI SUIT LA FONCTION QU'IL EPROUVE.** `ce_qui_tourne` sert a nommer ce que le
    /// backend faisait au moment d'un gel, ce qui n'a de sens que dans la boucle du
    /// guetteur : elle vit donc derriere la feature, et l'essai aussi. Sans cette ligne, la
    /// suite ne COMPILE PAS dans la configuration livree, celle sans Tauri.
    #[cfg(feature = "interface-tauri")]
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

    // --- La reparation ---

    fn etat(panne: Option<Panne>, tours_en_panne: u32) -> Etat {
        Etat {
            panne,
            tours_en_panne,
            tours_depuis_entree: 0,
            rechargements_fenetre: 0,
            tours_depuis_rechargement: u64::MAX,
        }
    }

    #[test]
    fn sans_panne_rien_ne_se_passe() {
        assert_eq!(prochaine_action(&etat(None, 0)), Action::Rien);
    }

    /// **LE FAUX POSITIF DE LA NUIT.** Ecran eteint ou session verrouillee : la fenetre reste
    /// « visible », rien n'est peint, personne devant. Sans la porte de presence, le guetteur
    /// rechargerait puis relancerait l'application toute la nuit. Retirer la condition de
    /// presence dans `prochaine_action` DOIT faire tomber cet essai.
    #[test]
    fn sans_utilisateur_devant_la_fenetre_rien_ne_se_passe() {
        let absente = Etat {
            tours_depuis_entree: TOURS_SANS_PRESENCE + 1,
            ..etat(Some(Panne::RenduArrete), TOURS_AVANT_RELANCER_RENDU * 2)
        };
        assert_eq!(prochaine_action(&absente), Action::Rien);
        let muette_absente = Etat {
            panne: Some(Panne::PageMuette),
            ..absente
        };
        assert_eq!(prochaine_action(&muette_absente), Action::Rien);
    }

    /// La boucle figee se passe de la presence : la page ne peut justement plus la rapporter.
    /// Une minute reelle de boucle morte, et l'application se relance — meme sans entree
    /// recente. Retirer la dispense de presence pour `BoucleFigee` DOIT faire tomber cet essai.
    #[test]
    fn la_boucle_figee_relance_meme_sans_nouvelle_de_la_page() {
        let figee = Etat {
            tours_depuis_entree: TOURS_SANS_PRESENCE * 10,
            ..etat(Some(Panne::BoucleFigee), TOURS_AVANT_RELANCER_BOUCLE)
        };
        assert_eq!(prochaine_action(&figee), Action::Relancer);
        let pas_encore = Etat {
            tours_en_panne: TOURS_AVANT_RELANCER_BOUCLE - 1,
            ..figee
        };
        assert_eq!(prochaine_action(&pas_encore), Action::Rien);
    }

    #[test]
    fn le_rendu_gele_recharge_la_vue_si_l_utilisateur_est_la() {
        // La confirmation prend deja ses tours ; la marge ecarte un hoquet de rendu.
        let trop_tot = etat(Some(Panne::RenduArrete), TOURS_AVANT_RECHARGEMENT - 1);
        assert_eq!(prochaine_action(&trop_tot), Action::Rien);
        let mure = etat(Some(Panne::RenduArrete), TOURS_AVANT_RECHARGEMENT);
        assert_eq!(prochaine_action(&mure), Action::Recharger);
        // La page muette se soigne pareil : recharger ne demande pas son JavaScript.
        let muette = etat(Some(Panne::PageMuette), TOURS_AVANT_RECHARGEMENT);
        assert_eq!(prochaine_action(&muette), Action::Recharger);
    }

    /// Un affichage qui gele toutes les minutes ne doit pas transformer l'application en
    /// boucle de rechargement : un delai minimal separe deux rechargements.
    #[test]
    fn deux_rechargements_sont_separes_par_le_delai() {
        let recent = Etat {
            tours_depuis_rechargement: TOURS_ENTRE_RECHARGEMENTS - 1,
            ..etat(Some(Panne::RenduArrete), TOURS_AVANT_RECHARGEMENT * 4)
        };
        assert_eq!(prochaine_action(&recent), Action::Rien);
        let ancien = Etat {
            tours_depuis_rechargement: TOURS_ENTRE_RECHARGEMENTS,
            ..recent
        };
        assert_eq!(prochaine_action(&ancien), Action::Recharger);
    }

    #[test]
    fn recharger_trois_fois_sans_succes_mene_a_la_relance() {
        let epuise = Etat {
            rechargements_fenetre: RECHARGEMENTS_MAX,
            tours_en_panne: TOURS_AVANT_RELANCER_RENDU,
            ..etat(Some(Panne::RenduArrete), 0)
        };
        assert_eq!(prochaine_action(&epuise), Action::Relancer);
        // Epuise mais la panne est jeune : elle peut encore se recuperer seule.
        let jeune = Etat {
            tours_en_panne: TOURS_AVANT_RELANCER_RENDU - 1,
            ..epuise
        };
        assert_eq!(prochaine_action(&jeune), Action::Rien);
        // Moins de trois rechargements : on continue de recharger, pas de relancer.
        let pas_epuise = Etat {
            rechargements_fenetre: RECHARGEMENTS_MAX - 1,
            ..epuise
        };
        assert_eq!(prochaine_action(&pas_epuise), Action::Recharger);
    }

    /// La proposition de mode secours se fait AU RETOUR A LA NORMALE : un dialogue ne se lit
    /// pas sur un ecran qui ne peint plus. Et une seule fois par lancement.
    #[test]
    fn le_secours_est_propose_apres_trois_episodes_et_une_seule_fois() {
        assert!(!faut_il_proposer_le_secours(
            Some(Panne::RenduArrete),
            EPISODES_AVANT_PROPOSITION - 1,
            false
        ));
        assert!(faut_il_proposer_le_secours(
            Some(Panne::RenduArrete),
            EPISODES_AVANT_PROPOSITION,
            false
        ));
        assert!(!faut_il_proposer_le_secours(
            Some(Panne::RenduArrete),
            EPISODES_AVANT_PROPOSITION,
            true
        ));
        // Une boucle figee n'est PAS le moteur de rendu : le mode secours n'y changerait
        // rien, le proposer serait mentir.
        assert!(!faut_il_proposer_le_secours(
            Some(Panne::BoucleFigee),
            EPISODES_AVANT_PROPOSITION * 2,
            false
        ));
    }

    #[test]
    fn la_fenetre_d_observation_oublie_ce_qui_est_vieux() {
        let maintenant = 10_000u64;
        let mut file: VecDeque<u64> =
            [maintenant - FENETRE_OBSERVATION - 1, maintenant - 10, maintenant]
                .into_iter()
                .collect();
        tailler_la_fenetre(&mut file, maintenant);
        assert_eq!(file.len(), 2, "l'evenement trop vieux devait partir");
        // La borne est inclusive : exactement la fenetre, on garde.
        let mut borne: VecDeque<u64> = [maintenant - FENETRE_OBSERVATION].into_iter().collect();
        tailler_la_fenetre(&mut borne, maintenant);
        assert_eq!(borne.len(), 1);
    }
}
