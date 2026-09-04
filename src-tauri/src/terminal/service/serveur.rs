//! Le service : il ecoute, il tient les shells, il ne connait pas Cockpit.
//!
//! Un processus par utilisateur, qui survit a la fermeture de l'application. Il n'ecrit
//! RIEN sur disque : tout est en memoire et meurt avec la machine. C'est exactement le
//! besoin (survivre a l'application, pas au redemarrage) et ca supprime toute question de
//! migration de format.
//!
//! ## Qui detient quoi
//!
//! Le service tient l'etat VIVANT : les sessions, leur taille, leur ecran, ce qui tourne
//! dedans. SQLite garde le nom d'onglet et le projet, parce que ca doit survivre a un
//! redemarrage de la machine — alors que le service, non. C'est pour la meme raison que
//! **l'identifiant d'un terminal vient de l'application** (`Requete::Creer` le porte) :
//! le rowid SQLite est la seule chose qui traverse un reboot.
//!
//! ## Un thread par role, pas de runtime asynchrone
//!
//! Par connexion : un lecteur (les requetes) et un ecrivain (la file de sortie). Par
//! session : un lecteur de PTY et un emetteur. Une dizaine de terminaux fait une
//! quarantaine de threads endormis — moins cher qu'un second runtime a cote de celui de
//! Tauri, et surtout : le chemin de frappe reste une ecriture directe, sans passer par
//! une file de taches.

use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use interprocess::local_socket::Stream;

use super::protocole::{
    self, InfoSession, Position, Pousse, Reponse, Requete, Taille, Trame,
};
use super::session::{Destinataire, Session};
use super::tuyau;

/// Nombre de lignes d'historique d'un terminal de largeur ordinaire. C'est ce que la
/// molette remonte aujourd'hui, et ca ne bouge pas.
pub use crate::terminal::ecran::HISTORIQUE;

/// Plafond de CELLULES gardees en historique pour une session.
///
/// L'historique se paie en cellules, pas en lignes : une cellule d'alacritty pese 24
/// octets, et une ligne en contient autant que le terminal a de colonnes. Compter en
/// lignes seulement laisse la facture suivre la largeur de la fenetre — un terminal de
/// 240 colonnes coute trois fois celui de 80 pour le meme « 10 000 lignes ».
///
/// 800 000 cellules = 10 000 lignes a 80 colonnes : la promesse tenue telle quelle a
/// largeur ordinaire, et la facture bornee au-dela. Mesures du 2026-08-21 sur cette
/// machine, en `--release`, cout d'une session SUPPLEMENTAIRE :
///
/// | Session | A vide | Historique plein |
/// |---|---|---|
/// | 80 colonnes, 10 000 lignes | 204 Ko | 19,5 Mo |
/// | 240 colonnes, 3 333 lignes (avec ce plafond) | 320 Ko | 23,1 Mo |
/// | 240 colonnes, 10 000 lignes (sans ce plafond) | 320 Ko | **57,1 Mo** |
///
/// C'est cette derniere ligne qui a decide du plafond : la meme fenetre, simplement plus
/// large, triplait la facture pour un historique que personne n'avait demande plus long.
///
/// Ce qui reste assume : onze terminaux de 80 colonnes REELLEMENT pleins font ~215 Mo.
/// C'est le pire cas, et il n'est presque jamais atteint — alacritty n'alloue les lignes
/// QU'AU FUR ET A MESURE qu'elles defilent (204 Ko pour une session neuve contre 19,5 Mo
/// une fois pleine), donc un terminal ou tourne un agent en plein ecran ne coute rien.
/// Le prochain levier, s'il faut descendre plus bas, est de ranger les lignes de
/// l'historique autrement que la grille vive (texte + attributs comprimes) : gros
/// chantier, a ne lancer que sur une plainte reelle.
pub const CELLULES_D_HISTORIQUE: usize = 800_000;

/// Plancher : meme tres large, un terminal garde de quoi remonter.
const HISTORIQUE_MINIMUM: usize = 2_000;

/// Plafond d'une photographie de terminal, historique compris.
///
/// 1 Mo : de quoi porter un ecran et un long defilement de texte, et assez petit pour
/// qu'un projet a huit terminaux n'ajoute pas dix mega-octets a la base a chaque
/// enregistrement. Au-dela, seul l'ecran visible est garde (voir `instantane`).
const INSTANTANE_MAX: usize = 1024 * 1024;

/// Combien de lignes d'historique pour un terminal de cette largeur.
pub fn lignes_d_historique(colonnes: u16, plafond: usize) -> usize {
    let par_les_cellules = CELLULES_D_HISTORIQUE / (colonnes.max(1) as usize);
    par_les_cellules.clamp(HISTORIQUE_MINIMUM.min(plafond), plafond)
}

/// L'etat du service.
pub struct Service {
    sessions: Mutex<HashMap<i64, Arc<Session>>>,
    /// Plafond de lignes d'historique. Le nombre reel depend de la largeur du terminal,
    /// voir `lignes_d_historique`.
    historique: usize,
    /// Chemin du socket : sert a se reveiller soi-meme quand on demande l'arret.
    chemin: std::path::PathBuf,
    arret: AtomicBool,
    numero_suivant: AtomicU64,
}

impl Service {
    fn neuf(chemin: std::path::PathBuf, historique: usize) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            historique,
            chemin,
            arret: AtomicBool::new(false),
            numero_suivant: AtomicU64::new(1),
        }
    }

    fn session(&self, id: i64) -> Result<Arc<Session>, String> {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("terminal {id} inconnu du service"))
    }
}

/// Ecoute jusqu'a ce qu'on demande l'arret. Rend la main quand le service s'arrete.
///
/// Ne rend PAS d'erreur si une connexion se passe mal : une connexion fautive ne doit pas
/// emporter les terminaux des autres. Seul l'echec d'ouverture du socket est fatal.
pub fn servir(chemin: &std::path::Path, historique: usize) -> std::io::Result<()> {
    let ecoute = tuyau::ecouter(chemin)?;
    let service = Arc::new(Service::neuf(chemin.to_path_buf(), historique));
    for flux in ecoute {
        if service.arret.load(Ordering::SeqCst) {
            break;
        }
        let Ok(flux) = flux else { continue };
        let service = Arc::clone(&service);
        std::thread::spawn(move || servir_une_connexion(&service, flux));
    }
    // Les shells meurent avec le service : c'est la regle annoncee, ils ne survivent pas
    // au redemarrage de la machine non plus.
    for (_, session) in service.sessions.lock().unwrap_or_else(|e| e.into_inner()).drain() {
        let _ = session.fermer();
    }
    Ok(())
}

/// La file de sortie d'une connexion, et le thread qui l'ecoule vers le socket.
///
/// Une file plutot qu'une ecriture directe : le thread qui produit (l'emetteur d'une
/// session, ou le lecteur de requetes) ne doit jamais se retrouver bloque parce que le
/// client ne lit pas assez vite. L'ordre est strict par construction — une seule file, un
/// seul preneur.
pub struct Connexion {
    numero: u64,
    file: Mutex<Option<std::collections::VecDeque<Vec<u8>>>>,
    signal: Condvar,
    /// Terminaux sur lesquels cette connexion s'est branchee : a detacher en partant.
    branchee_sur: Mutex<HashSet<i64>>,
}

impl Connexion {
    fn neuve(numero: u64) -> Self {
        Self {
            numero,
            file: Mutex::new(Some(std::collections::VecDeque::new())),
            signal: Condvar::new(),
            branchee_sur: Mutex::new(HashSet::new()),
        }
    }

    fn envoyer(&self, trame: Trame) -> bool {
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        let Some(file) = file.as_mut() else { return false };
        file.push_back(trame.encoder());
        self.signal.notify_one();
        true
    }

    fn fermer(&self) {
        *self.file.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.signal.notify_all();
    }

    /// Attend la trame suivante. `None` = la connexion est fermee.
    fn prochaine(&self) -> Option<Vec<u8>> {
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            match file.as_mut() {
                None => return None,
                Some(f) => {
                    if let Some(trame) = f.pop_front() {
                        return Some(trame);
                    }
                }
            }
            file = self.signal.wait(file).unwrap_or_else(|e| e.into_inner());
        }
    }
}

impl Destinataire for Connexion {
    fn pousser(&self, pousse: Pousse) -> bool {
        self.envoyer(Trame::Pousse(pousse))
    }

    fn numero(&self) -> u64 {
        self.numero
    }
}

fn servir_une_connexion(service: &Arc<Service>, flux: Stream) {
    let flux = Arc::new(flux);
    // Le service parle en PREMIER : dix octets de forme figee qui portent la version du
    // protocole. Voir l'en-tete de protocole.rs.
    if protocole::ecrire_preambule(&mut (&*flux)).is_err() {
        return;
    }
    // Un socket est propre a un utilisateur, mais rien n'empeche un autre compte d'avoir
    // obtenu le descripteur : on verifie avant d'ouvrir le moindre shell.
    if let Err(refus) = tuyau::verifier_pair(&flux) {
        let _ = (&*flux).write_all(
            &Trame::Pousse(Pousse::Panne { id: 0, message: refus }).encoder(),
        );
        return;
    }

    let connexion = Arc::new(Connexion::neuve(
        service.numero_suivant.fetch_add(1, Ordering::SeqCst),
    ));

    let ecrivain = {
        let connexion = Arc::clone(&connexion);
        let flux = Arc::clone(&flux);
        std::thread::spawn(move || {
            while let Some(trame) = connexion.prochaine() {
                if (&*flux).write_all(&trame).is_err() {
                    break;
                }
            }
            connexion.fermer();
        })
    };

    let mut lecteur = BufReader::new(&*flux);
    loop {
        match Trame::lire(&mut lecteur) {
            Ok(Some(Trame::Requete { sequence, requete })) => {
                let arret = matches!(requete, Requete::Arreter);
                if let Some(reponse) = traiter(service, &connexion, requete) {
                    connexion.envoyer(Trame::Reponse { sequence, reponse });
                }
                if arret {
                    break;
                }
            }
            // Une reponse ou une poussee venant du client n'a aucun sens : on ignore
            // plutot que de couper, le flux reste synchrone.
            Ok(Some(_)) => continue,
            Ok(None) => break,
            Err(_) => break,
        }
    }

    connexion.fermer();
    let _ = ecrivain.join();
    // Le client s'en va : les shells CONTINUENT, on se contente de ne plus rien leur
    // envoyer. C'est toute la promesse du service.
    for id in connexion.branchee_sur.lock().unwrap_or_else(|e| e.into_inner()).iter() {
        if let Ok(session) = service.session(*id) {
            session.detacher_si(connexion.numero);
        }
    }
    if service.arret.load(Ordering::SeqCst) {
        // Reveille la boucle d'acceptation, qui dort sur `accept()` : sans ce coup de
        // sonnette, le service ne s'arreterait qu'a la connexion suivante.
        let _ = tuyau::connecter(&service.chemin);
    }
}

/// Traite une requete. `None` = pas de reponse a envoyer (chemin de frappe).
fn traiter(service: &Arc<Service>, connexion: &Arc<Connexion>, requete: Requete) -> Option<Reponse> {
    match requete {
        Requete::Creer { id, dossier, taille, commande_initiale, ecran_initial } => {
            Some(match creer(service, id, &dossier, taille, commande_initiale, &ecran_initial) {
                Ok(()) => Reponse::Fait,
                Err(e) => Reponse::Erreur(e),
            })
        }
        // AUCUNE reponse : un aller-retour par touche ajouterait la latence du tuyau au
        // chemin le plus chaud du projet. Un echec revient en `Pousse::Panne`.
        Requete::Ecrire { id, octets } => {
            if let Err(e) = service.session(id).and_then(|s| s.ecrire(&octets)) {
                connexion.pousser(Pousse::Panne { id, message: e });
            }
            None
        }
        Requete::Redimensionner { id, taille } => Some(reponse(
            service.session(id).and_then(|s| s.redimensionner(taille)),
        )),
        Requete::Fermer { id } => Some(reponse(fermer(service, id))),
        Requete::Attacher { id, taille } => Some(reponse(attacher(service, connexion, id, taille))),
        Requete::Lister => Some(Reponse::Sessions(lister(service))),
        Requete::Chercher { id, action, motif } => Some(
            match service.session(id).and_then(|s| s.chercher(action, &motif)) {
                Ok((total, index, ou)) => Reponse::Recherche {
                    total,
                    index,
                    occurrence: ou.map(|o| Position { ligne: o.ligne, colonne: o.colonne }),
                },
                Err(e) => Reponse::Erreur(e),
            },
        ),
        Requete::CopierSelection { id, debut, fin } => Some(match service.session(id) {
            Ok(s) => Reponse::Texte(s.texte_region(
                (debut.ligne, debut.colonne),
                (fin.ligne, fin.colonne),
            )),
            Err(e) => Reponse::Erreur(e),
        }),
        Requete::Redessiner { id, avec_historique } => Some(match service.session(id) {
            Ok(s) => {
                connexion.pousser(Pousse::Redessin { id, octets: s.redessin(avec_historique) });
                Reponse::Fait
            }
            Err(e) => Reponse::Erreur(e),
        }),
        Requete::Arreter => {
            service.arret.store(true, Ordering::SeqCst);
            Some(Reponse::Fait)
        }
        Requete::Instantane { id } => Some(match service.session(id) {
            Ok(s) => Reponse::Octets(instantane(&s)),
            Err(e) => Reponse::Erreur(e),
        }),
    }
}

/// La photographie d'un terminal, BORNEE en octets.
///
/// L'ecran et son historique d'abord : c'est ce qui rend un terminal « comme on l'a
/// quitte », molette comprise. Mais l'historique peut faire des mega-octets, et cette
/// photo finit dans la base de l'application : au-dela du plafond, on rend l'ecran seul.
/// Perdre le defilement est moins grave que faire grossir la base sans limite.
fn instantane(session: &Session) -> Vec<u8> {
    let avec_historique = session.redessin(true);
    if avec_historique.len() <= INSTANTANE_MAX {
        return avec_historique;
    }
    session.redessin(false)
}

fn reponse(resultat: Result<(), String>) -> Reponse {
    match resultat {
        Ok(()) => Reponse::Fait,
        Err(e) => Reponse::Erreur(e),
    }
}

fn creer(
    service: &Arc<Service>,
    id: i64,
    dossier: &str,
    taille: Taille,
    commande_initiale: Option<String>,
    ecran_initial: &[u8],
) -> Result<(), String> {
    // Un identifiant deja pris signifie que l'application et le service ne sont pas
    // d'accord sur ce qui existe. Refuser plutot qu'ecraser : ecraser tuerait un shell
    // vivant sans que personne l'ait demande.
    if service.sessions.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&id) {
        return Err(format!("le terminal {id} existe deja dans le service"));
    }
    let historique = lignes_d_historique(taille.colonnes, service.historique);
    let session = Session::ouvrir(id, dossier, taille, commande_initiale, historique, ecran_initial)?;
    service
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id, session);
    Ok(())
}

fn fermer(service: &Arc<Service>, id: i64) -> Result<(), String> {
    let session = service
        .sessions
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id)
        .ok_or_else(|| format!("terminal {id} inconnu du service"))?;
    session.fermer()
}

fn attacher(
    service: &Arc<Service>,
    connexion: &Arc<Connexion>,
    id: i64,
    taille: Taille,
) -> Result<(), String> {
    let session = service.session(id)?;
    // La taille AVANT l'attache : le redessin qui suit doit decrire la grille telle que le
    // frontend l'affiche, sinon le premier dessin est a l'ancienne taille et le terminal
    // se recadre sous les yeux de l'utilisateur.
    session.redimensionner(taille)?;
    connexion.branchee_sur.lock().unwrap_or_else(|e| e.into_inner()).insert(id);
    session.attacher(Arc::clone(connexion) as Arc<dyn Destinataire>);
    Ok(())
}

/// Les sessions du service, et le ramassage des mortes.
///
/// C'est ici qu'une session dont le shell s'est termine est signalee une derniere fois
/// (`vivant: false`) puis oubliee : l'application y lit de quoi supprimer sa ligne SQLite.
/// Sans ce passage, un terminal ferme resterait dans la liste jusqu'a l'arret du service.
fn lister(service: &Arc<Service>) -> Vec<InfoSession> {
    // UN SEUL arbre de process pour toute la passe : c'est ce qui rend la detection
    // d'agents bon marche (0,35 ms au lieu de 47 ms, mesure du 2026-08-20).
    let arbre = crate::terminal::agents_llm::ArbreProcess::nouveau();
    let mut sessions = service.sessions.lock().unwrap_or_else(|e| e.into_inner());
    let mut infos: Vec<InfoSession> = sessions
        .values()
        .map(|s| InfoSession {
            id: s.id,
            vivant: s.vivant(),
            llm: s.vivant() && s.llm(&arbre),
            taille: s.taille(),
        })
        .collect();
    infos.sort_by_key(|i| i.id);
    sessions.retain(|_, s| s.vivant());
    infos
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le ramassage : une session morte est annoncee une fois, puis oubliee.
    #[test]
    fn une_session_morte_est_signalee_puis_oubliee() {
        let service = Arc::new(Service::neuf(std::path::PathBuf::from("/inutilise"), 100));
        creer(&service, 7, &std::env::temp_dir().to_string_lossy(), Taille { colonnes: 40, lignes: 10 }, Some("exit".into()), &[])
            .unwrap();

        let debut = std::time::Instant::now();
        loop {
            let infos = lister(&service);
            match infos.first() {
                Some(info) if !info.vivant => break,
                _ => {}
            }
            assert!(debut.elapsed() < std::time::Duration::from_secs(20), "le shell ne meurt pas");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(lister(&service).is_empty(), "la session morte aurait du etre oubliee");
    }

    /// Un identifiant deja pris ne doit pas tuer le shell qui l'occupe.
    #[test]
    fn creer_deux_fois_le_meme_identifiant_est_refuse() {
        let service = Arc::new(Service::neuf(std::path::PathBuf::from("/inutilise"), 100));
        let dossier = std::env::temp_dir().to_string_lossy().to_string();
        let taille = Taille { colonnes: 40, lignes: 10 };
        creer(&service, 1, &dossier, taille, None, &[]).unwrap();
        let erreur = creer(&service, 1, &dossier, taille, None, &[]).unwrap_err();
        assert!(erreur.contains("existe deja"), "{erreur}");
        assert_eq!(lister(&service).len(), 1);
        fermer(&service, 1).unwrap();
    }

    /// La promesse tenue a largeur ordinaire, et la facture bornee quand la fenetre
    /// s'elargit : c'est tout l'objet du plafond en cellules.
    #[test]
    fn l_historique_tient_la_promesse_a_80_colonnes_et_se_borne_au_dela() {
        assert_eq!(lignes_d_historique(80, HISTORIQUE), HISTORIQUE);
        // Plus large : moins de lignes, mais autant de cellules — donc la meme memoire.
        assert_eq!(lignes_d_historique(240, HISTORIQUE), CELLULES_D_HISTORIQUE / 240);
        for colonnes in [40u16, 80, 120, 200, 240, 400] {
            let cellules = lignes_d_historique(colonnes, HISTORIQUE) * colonnes as usize;
            assert!(
                cellules <= CELLULES_D_HISTORIQUE.max(HISTORIQUE_MINIMUM * colonnes as usize),
                "{colonnes} colonnes : {cellules} cellules"
            );
        }
        // Un terminal demesurement large garde quand meme de quoi remonter.
        assert_eq!(lignes_d_historique(2000, HISTORIQUE), HISTORIQUE_MINIMUM);
        // Et un plafond bas (les essais) l'emporte toujours.
        assert_eq!(lignes_d_historique(80, 500), 500);
    }

    #[test]
    fn une_operation_sur_un_terminal_inconnu_est_une_erreur_explicite() {
        let service = Arc::new(Service::neuf(std::path::PathBuf::from("/inutilise"), 100));
        let erreur = service.session(404).err().expect("404 n'existe pas");
        assert!(erreur.contains("404") && erreur.contains("inconnu"), "{erreur}");
        assert!(fermer(&service, 404).is_err());
    }
}
