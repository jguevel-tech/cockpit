//! Le cote application du tuyau : parler au service de terminaux.
//!
//! Ce module ne connait ni Tauri, ni SQLite. L'adaptateur qui implemente le trait
//! `Terminaux` par-dessus arrive a l'etape C du chantier
//! (`docs/portabilite/plan-terminaux.md`) ; ici on ne fait que la conversation.
//!
//! **Les frappes ne font pas d'aller-retour.** `ecrire()` pose la trame sur le socket et
//! rend la main : attendre un accuse ajouterait un aller-retour a chaque touche. Un echec
//! d'ecriture revient par la poussee `Panne`, donc rien n'est avale en silence.

use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use interprocess::local_socket::traits::Stream as _;
use interprocess::local_socket::Stream;

use super::protocole::{
    self, ActionRecherche, ErreurPoignee, InfoSession, Position, Pousse, Reponse, Requete,
    Taille, Trame,
};
use super::tuyau;

/// Au-dela, on considere que le service ne repond plus a une question.
///
/// Ce n'est PAS une borne de service lent : le service tient tout en memoire et repond sur un
/// socket local, donc une reponse legitime arrive en quelques millisecondes — meme un redessin
/// qui porte tout l'historique. C'est une borne de service COINCE. Elle existe parce que la
/// frappe partage un verrou avec ces appels : sans borne, la fenetre gele pour de bon, et seul
/// un kill en sort (essai `un_service_qui_n_accuse_jamais_reception_ne_gele_pas_l_application`).
const DELAI_REPONSE: Duration = Duration::from_secs(5);

/// Au-dela, personne n'ecrit le preambule au bout de ce socket.
///
/// Le service ecrit ses dix octets des l'acceptation. Trois secondes couvrent une machine
/// chargee et refusent le cas qui gele : un processus arrete net laisse le noyau etablir la
/// connexion, et plus aucun fil n'ecrit jamais.
const DELAI_POIGNEE: Duration = Duration::from_secs(3);

/// Une conversation ouverte avec le service.
pub struct Client {
    flux: Arc<Stream>,
    /// Un seul ecrivain a la fois : deux trames entrelacees seraient illisibles.
    ecriture: Mutex<()>,
    sequence: AtomicU32,
    attentes: Mutex<HashMap<u32, Sender<Reponse>>>,
    ferme: Arc<AtomicBool>,
}

impl Client {
    /// Se connecte, verifie la poignee de main et l'identite du service, puis met en route
    /// le thread qui lit ce qui arrive.
    ///
    /// `sur_poussee` recoit tout ce que le service envoie de lui-meme : la sortie des
    /// terminaux, les redessins, le presse-papier, les fins et les pannes.
    pub fn connecter(
        chemin: &std::path::Path,
        sur_poussee: impl Fn(Pousse) + Send + 'static,
    ) -> Result<Arc<Self>, ErreurPoignee> {
        let flux = Arc::new(tuyau::connecter(chemin)?);
        // Sans borne, cette lecture ne rend JAMAIS la main quand le noyau a etabli la
        // connexion mais que plus aucun fil du service n'ecrit. Windows n'offre pas ce
        // reglage — `interprocess` refuse tout delai sur un tuyau nomme — d'ou le `let _`
        // plutot qu'un `?` : la-bas la protection est le `try_lock` de l'adaptateur, qui
        // empeche la boucle graphique d'attendre derriere cette poignee de main.
        let _ = flux.set_recv_timeout(Some(DELAI_POIGNEE));
        // Le service parle en premier : on lit sa version AVANT d'envoyer quoi que ce
        // soit. C'est ce qui permet de dire « ce service est plus ancien que moi » au lieu
        // d'echouer sur un message incomprehensible.
        protocole::lire_preambule(&mut (&*flux))?;
        // Passe la poignee de main, l'ecoute doit pouvoir attendre sans fin : un terminal
        // silencieux n'est pas une panne, et un delai ferait rompre la connexion.
        let _ = flux.set_recv_timeout(None);
        // Et on refuse de confier des frappes a un service qui n'est pas le notre.
        tuyau::verifier_pair(&flux).map_err(ErreurPoignee::Tuyau)?;

        let client = Arc::new(Self {
            flux: Arc::clone(&flux),
            ecriture: Mutex::new(()),
            sequence: AtomicU32::new(1),
            attentes: Mutex::new(HashMap::new()),
            ferme: Arc::new(AtomicBool::new(false)),
        });

        {
            let client = Arc::clone(&client);
            std::thread::spawn(move || {
                let mut lecteur = BufReader::new(&*flux);
                loop {
                    match Trame::lire(&mut lecteur) {
                        Ok(Some(Trame::Reponse { sequence, reponse })) => {
                            if let Some(qui_attend) =
                                client.attentes.lock().unwrap_or_else(|e| e.into_inner()).remove(&sequence)
                            {
                                let _ = qui_attend.send(reponse);
                            }
                        }
                        Ok(Some(Trame::Pousse(pousse))) => sur_poussee(pousse),
                        // Une requete venue du service n'existe pas : on ignore sans
                        // couper, le flux reste synchrone.
                        Ok(Some(Trame::Requete { .. })) => continue,
                        Ok(None) | Err(_) => break,
                    }
                }
                // Le service est parti : personne ne doit rester bloque sur une reponse
                // qui n'arrivera jamais. Fermer les canaux reveille tous les appelants.
                client.ferme.store(true, Ordering::SeqCst);
                client.attentes.lock().unwrap_or_else(|e| e.into_inner()).clear();
            });
        }

        Ok(client)
    }

    /// Le service repond-il toujours ?
    pub fn vivant(&self) -> bool {
        !self.ferme.load(Ordering::SeqCst)
    }

    fn envoyer(&self, sequence: u32, requete: Requete) -> Result<(), String> {
        let trame = Trame::Requete { sequence, requete }.encoder();
        let _ecriture = self.ecriture.lock().map_err(|_| "tuyau du service perdu")?;
        (&*self.flux).write_all(&trame).map_err(|e| {
            // Une ecriture qui echoue a pu partir A MOITIE : le service lit alors une trame
            // tronquee et tout ce qui suit est decale. Cette connexion ne se rattrape pas,
            // on la declare morte pour que la suivante soit neuve.
            self.ferme.store(true, Ordering::SeqCst);
            e.to_string()
        })
    }

    /// Envoie une requete et attend sa reponse.
    fn demander(&self, requete: Requete) -> Result<Reponse, String> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        self.attentes.lock().unwrap_or_else(|e| e.into_inner()).insert(sequence, tx);
        if let Err(e) = self.envoyer(sequence, requete) {
            self.attentes.lock().unwrap_or_else(|e| e.into_inner()).remove(&sequence);
            return Err(e);
        }
        match rx.recv_timeout(DELAI_REPONSE) {
            Ok(reponse) => Ok(reponse),
            Err(_) => {
                // On retire l'attente : sans ca, une reponse tardive resterait dans la
                // table pour toujours et celle-ci grossirait a chaque appel perdu.
                self.attentes.lock().unwrap_or_else(|e| e.into_inner()).remove(&sequence);
                Err("le service de terminaux ne repond plus".to_string())
            }
        }
    }

    /// Les reponses qui ne portent qu'un succes ou une erreur.
    fn faire(&self, requete: Requete) -> Result<(), String> {
        match self.demander(requete)? {
            Reponse::Fait => Ok(()),
            Reponse::Erreur(e) => Err(e),
            autre => Err(format!("reponse inattendue du service : {autre:?}")),
        }
    }

    pub fn creer(
        &self,
        id: i64,
        dossier: &str,
        taille: Taille,
        commande_initiale: Option<String>,
    ) -> Result<(), String> {
        self.faire(Requete::Creer {
            id,
            dossier: dossier.to_string(),
            taille,
            commande_initiale,
        })
    }

    /// Le chemin de frappe : une trame posee sur le socket, pas d'attente.
    pub fn ecrire(&self, id: i64, octets: &[u8]) -> Result<(), String> {
        // Le numero de sequence est consomme quand meme : il n'y a pas de reponse a
        // apparier, mais un numero reutilise brouillerait une trace de diagnostic.
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);
        self.envoyer(sequence, Requete::Ecrire { id, octets: octets.to_vec() })
    }

    pub fn redimensionner(&self, id: i64, taille: Taille) -> Result<(), String> {
        self.faire(Requete::Redimensionner { id, taille })
    }

    pub fn fermer(&self, id: i64) -> Result<(), String> {
        self.faire(Requete::Fermer { id })
    }

    /// Branche cette connexion sur la sortie du terminal. Le redessin arrive juste apres,
    /// par le canal des poussees — cette fonction ne rend rien, comme le trait de l'etape A.
    pub fn attacher(&self, id: i64, taille: Taille) -> Result<(), String> {
        self.faire(Requete::Attacher { id, taille })
    }

    pub fn lister(&self) -> Result<Vec<InfoSession>, String> {
        match self.demander(Requete::Lister)? {
            Reponse::Sessions(sessions) => Ok(sessions),
            Reponse::Erreur(e) => Err(e),
            autre => Err(format!("reponse inattendue du service : {autre:?}")),
        }
    }

    /// Un geste de recherche. Rend (nombre d'occurrences, indice courant, position).
    pub fn chercher(
        &self,
        id: i64,
        action: ActionRecherche,
        motif: &str,
    ) -> Result<(u32, Option<u32>, Option<Position>), String> {
        match self.demander(Requete::Chercher { id, action, motif: motif.to_string() })? {
            Reponse::Recherche { total, index, occurrence } => Ok((total, index, occurrence)),
            Reponse::Erreur(e) => Err(e),
            autre => Err(format!("reponse inattendue du service : {autre:?}")),
        }
    }

    /// Le texte d'une region, bornes comprises.
    ///
    /// PAS D'APPELANT dans l'application : la selection a la souris appartient a xterm, qui
    /// tient desormais tout l'historique et rend son texte sans aller-retour. Gardee parce
    /// que le client doit couvrir le protocole du service — et parce qu'elle est la seule a
    /// pouvoir lire au-dela de ce que le terminal du frontend garde. Exercee par `tests.rs`.
    #[allow(dead_code)]
    pub fn copier_selection(
        &self,
        id: i64,
        debut: Position,
        fin: Position,
    ) -> Result<String, String> {
        match self.demander(Requete::CopierSelection { id, debut, fin })? {
            Reponse::Texte(texte) => Ok(texte),
            Reponse::Erreur(e) => Err(e),
            autre => Err(format!("reponse inattendue du service : {autre:?}")),
        }
    }

    /// Redemande l'etat complet. PAS D'APPELANT : l'attache en envoie deja un, et le
    /// frontend n'a jamais besoin d'en redemander un. Exercee par `tests.rs`.
    #[allow(dead_code)]
    pub fn redessiner(&self, id: i64, avec_historique: bool) -> Result<(), String> {
        self.faire(Requete::Redessiner { id, avec_historique })
    }

    /// Arrete le service. Les shells meurent avec lui — l'application ne le fait JAMAIS,
    /// c'est toute la promesse. Exercee par `tests.rs`.
    #[allow(dead_code)]
    pub fn arreter(&self) -> Result<(), String> {
        self.faire(Requete::Arreter)
    }
}
