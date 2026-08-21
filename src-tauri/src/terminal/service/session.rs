//! Un terminal, cote service : un shell dans un PTY, et l'ecran qu'il dessine.
//!
//! ## Ce qui part vers l'application, et pourquoi ce n'est pas le flux brut
//!
//! Un shell peut ecrire 4 Mo en une seconde ; l'ecran, lui, fait 1 280 octets a redessiner
//! (mesure de l'etape B1). Retransmettre tout ce que le shell ecrit ferait dessiner au
//! frontend quatre mille fois ce qu'il finira par afficher — c'est le service que tmux
//! rendait sans qu'on le sache, en ecrasant les lignes qui avaient defile.
//!
//! La regle appliquee ici :
//! - **ce qui est petit part TEL QUEL, tout de suite** : l'echo d'une touche fait quelques
//!   octets, il ne doit ni attendre ni provoquer un repeint complet ;
//! - **ce qui est gros est REMPLACE par un redessin** : au-dela de ce qu'un ecran peut
//!   montrer, le contenu a forcement ete recouvert, et le redessin est a la fois plus
//!   court et plus juste.
//!
//! Le prix de ce choix, assume : les lignes qui defilent pendant une rafale n'arrivent
//! jamais au terminal du frontend. Elles ne sont pas perdues — le service garde
//! l'historique — mais le defilement a la molette devra le lui demander (`Redessiner`
//! avec historique), au lieu de compter sur le tampon du frontend. tmux avait exactement
//! la meme propriete.
//!
//! ## Ce qui ne passe PAS par ici
//!
//! Le chemin de frappe. `ecrire()` prend le verrou de l'ecrivain du PTY et rien d'autre :
//! ni le verrou de l'ecran (qu'une rafale peut tenir), ni la file de sortie, ni un
//! aller-retour avec l'application. C'est ce qui garde la latence sous celle de tmux.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize};

use super::protocole::{Pousse, Taille};
use crate::terminal::ecran::{Ecran, Sortant};
use crate::terminal::environnement;

/// Taille du tampon de lecture du PTY. Ne change RIEN a la latence : `read()` rend la main
/// des qu'UN octet est disponible, l'echo d'une touche part donc toujours seul.
const LECTURE_PTY: usize = 64 * 1024;

/// Fenetre d'accumulation, appliquee UNIQUEMENT en rafale (voir `emettre`). 8 ms : un
/// demi-rafraichissement d'ecran a 60 Hz, donc invisible, et assez pour qu'une seconde de
/// sortie soutenue tienne en ~125 envois au lieu de plusieurs milliers.
///
/// NE PAS monter a 20-50 ms « pour mieux regrouper » : ce serait payer en latence percue
/// ce qu'on gagne en volume. Et ne pas l'appliquer au premier lot apres un silence : c'est
/// l'echo d'une touche, il doit partir tel quel.
const FENETRE_RAFALE: std::time::Duration = std::time::Duration::from_millis(8);

/// Volume a partir duquel un lot est deja une RAFALE, meme s'il a fallu l'attendre.
///
/// Le rythme ne suffit pas a lui seul : sur une machine chargee, le shell produit par
/// a-coups et l'emetteur attend a chaque fois — il croit voir des echos alors qu'il voit
/// une rafale au ralenti (mesure du 2026-08-21 : 939 envois pour 1,3 Mo pendant que la
/// suite d'essais tournait, contre 158 a vide). 2 Ko est bien au-dessus de l'echo d'une
/// touche (quelques octets) et d'un redessin d'invite (quelques centaines), et bien en
/// dessous d'un ecran.
const SEUIL_LOT: usize = 2 * 1024;

/// Volume d'un seul lot au-dela duquel on renonce a transmettre et on redessine.
///
/// 256 Ko dans une fenetre de 8 ms, c'est 32 Mo/s : aucun affichage humain ne suit, le
/// contenu a forcement ete recouvert plusieurs fois, et le redessin est a la fois plus
/// court et plus juste. En dessous on transmet TOUT — c'est ce qui remplit le tampon de
/// defilement du terminal du frontend, donc ce qui fait marcher la molette sans aller
/// demander quoi que ce soit au service.
const VOLUME_INSOUTENABLE: usize = 256 * 1024;

/// A qui la sortie d'un terminal est remise. Implemente par une connexion cliente.
///
/// Un trait plutot que le type concret : la session n'a alors rien a savoir du socket ni
/// du protocole, ce qui la rend testable avec un destinataire en memoire.
pub trait Destinataire: Send + Sync {
    /// Rend `false` si la connexion est morte : la session se detache alors toute seule.
    fn pousser(&self, pousse: Pousse) -> bool;

    /// Numero unique de la connexion. Sert a ne detacher QUE si l'abonne est toujours
    /// celui qui s'en va : entre-temps une autre connexion a pu prendre sa place, et la
    /// detacher a sa place aveuglerait le terminal.
    fn numero(&self) -> u64;
}

/// L'etat partage entre le lecteur du PTY et l'emetteur.
struct Tampon {
    ecran: Ecran,
    /// Octets lus du shell et pas encore remis a l'application.
    en_attente: Vec<u8>,
    /// Un redessin complet est du (attache, bascule d'ecran alternatif, demande explicite).
    redessin_du: bool,
    /// Une rafale insoutenable a ete remplacee par un redessin d'ecran, donc le tampon de
    /// defilement du frontend a ete vide : des que le calme revient, on lui renvoie
    /// l'historique complet.
    historique_du: bool,
    /// Dernier etat connu de l'ecran alternatif, pour reperer la bascule.
    alternatif: bool,
    abonne: Option<Arc<dyn Destinataire>>,
    /// Le PTY est ferme : l'emetteur ecoule ce qui reste puis s'arrete.
    fini: bool,
    /// Commande a taper des que le shell aura montre qu'il lit (voir `lire_pty`).
    commande_initiale: Option<String>,
    recherche: Recherche,
}

/// L'etat de la recherche en cours dans ce terminal.
#[derive(Default)]
struct Recherche {
    motif: String,
    occurrences: Vec<crate::terminal::ecran::Occurrence>,
    /// Indice dans `occurrences`, du plus ancien au plus recent.
    courante: Option<usize>,
}

/// Un terminal detenu par le service.
pub struct Session {
    pub id: i64,
    /// Verrou PROPRE au chemin de frappe : jamais pris par le lecteur ni par l'emetteur.
    ecrivain: Mutex<Box<dyn Write + Send>>,
    maitre: Mutex<Box<dyn MasterPty + Send>>,
    tueur: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    /// Pid du shell : c'est la racine de l'arbre ou l'on cherche un agent IA.
    pid: Option<u32>,
    vivant: Arc<AtomicBool>,
    partage: Arc<(Mutex<Tampon>, Condvar)>,
    taille: Mutex<Taille>,
}

impl Session {
    /// Ouvre un shell et met en route les deux threads qui le servent.
    pub fn ouvrir(
        id: i64,
        dossier: &str,
        taille: Taille,
        commande_initiale: Option<String>,
        historique: usize,
    ) -> Result<Arc<Self>, String> {
        let systeme = portable_pty::native_pty_system();
        let paire = systeme
            .openpty(PtySize {
                rows: taille.lignes.max(1),
                cols: taille.colonnes.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("ouverture du PTY : {e}"))?;

        // Le shell de l'utilisateur ($SHELL, sinon celui de son compte ; %COMSPEC% sous
        // Windows). C'est portable-pty qui connait la regle de chaque systeme.
        let mut commande = CommandBuilder::new_default_prog();
        if std::path::Path::new(dossier).is_dir() {
            commande.cwd(dossier);
        }
        commande.env("TERM", "xterm-256color");
        environnement::appliquer_pty(&mut commande);

        let enfant = paire
            .slave
            .spawn_command(commande)
            .map_err(|e| format!("lancement du shell : {e}"))?;
        drop(paire.slave);

        let pid = enfant.process_id();
        let tueur = enfant.clone_killer();
        let mut lecteur = paire
            .master
            .try_clone_reader()
            .map_err(|e| format!("lecture du PTY : {e}"))?;
        let ecrivain = paire
            .master
            .take_writer()
            .map_err(|e| format!("ecriture du PTY : {e}"))?;

        let partage = Arc::new((
            Mutex::new(Tampon {
                ecran: Ecran::avec_historique(
                    taille.colonnes as usize,
                    taille.lignes as usize,
                    historique,
                ),
                en_attente: Vec::new(),
                redessin_du: false,
                historique_du: false,
                alternatif: false,
                abonne: None,
                fini: false,
                commande_initiale,
                recherche: Recherche::default(),
            }),
            Condvar::new(),
        ));

        let session = Arc::new(Self {
            id,
            ecrivain: Mutex::new(ecrivain),
            maitre: Mutex::new(paire.master),
            tueur: Mutex::new(tueur),
            pid,
            vivant: Arc::new(AtomicBool::new(true)),
            partage: Arc::clone(&partage),
            taille: Mutex::new(taille),
        });

        {
            let session_lecteur = Arc::clone(&session);
            std::thread::spawn(move || {
                let emetteur = {
                    let session = Arc::clone(&session_lecteur);
                    std::thread::spawn(move || emettre(&session))
                };
                lire_pty(&mut lecteur, &session_lecteur);
                session_lecteur.vivant.store(false, Ordering::SeqCst);
                {
                    let (tampon, signal) = &*session_lecteur.partage;
                    tampon.lock().unwrap_or_else(|e| e.into_inner()).fini = true;
                    signal.notify_all();
                }
                // L'emetteur finit d'ecouler ce qui reste AVANT que la fin soit annoncee,
                // sinon les derniers octets affiches sont perdus.
                let _ = emetteur.join();
                // Le shell est mort : on le ramasse, sinon il reste zombie tant que le
                // service tourne — et le service tourne des jours.
                let mut enfant = enfant;
                let _ = enfant.wait();
                if let Some(abonne) = session_lecteur.abonne() {
                    abonne.pousser(Pousse::Fini { id });
                }
            });
        }

        Ok(session)
    }

    /// Le chemin de frappe. Direct : un verrou, une ecriture, rien d'autre.
    pub fn ecrire(&self, octets: &[u8]) -> Result<(), String> {
        let mut ecrivain = self.ecrivain.lock().map_err(|_| "ecrivain du PTY perdu")?;
        ecrivain.write_all(octets).map_err(|e| e.to_string())
    }

    pub fn redimensionner(&self, taille: Taille) -> Result<(), String> {
        self.maitre
            .lock()
            .map_err(|_| "PTY perdu")?
            .resize(PtySize {
                rows: taille.lignes.max(1),
                cols: taille.colonnes.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
        *self.taille.lock().unwrap_or_else(|e| e.into_inner()) = taille;
        let (tampon, signal) = &*self.partage;
        let mut t = tampon.lock().unwrap_or_else(|e| e.into_inner());
        t.ecran.redimensionner(taille.colonnes as usize, taille.lignes as usize);
        // La grille a change de forme : ce qui etait en attente decrit l'ancienne, et le
        // frontend a lui aussi redimensionne son terminal. Un redessin remet les deux
        // d'accord d'un coup.
        t.en_attente.clear();
        t.redessin_du = true;
        signal.notify_all();
        Ok(())
    }

    pub fn taille(&self) -> Taille {
        *self.taille.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Branche une connexion sur la sortie. L'ancienne, s'il y en avait une, est remplacee.
    ///
    /// Tout se fait sous LE MEME verrou que l'ingestion : aucun octet ne peut se glisser
    /// entre la photo de l'ecran et la reprise du flux, ni etre envoye deux fois.
    pub fn attacher(&self, abonne: Arc<dyn Destinataire>) {
        let (tampon, signal) = &*self.partage;
        let mut t = tampon.lock().unwrap_or_else(|e| e.into_inner());
        t.abonne = Some(abonne);
        t.en_attente.clear();
        t.redessin_du = true;
        signal.notify_all();
    }

    /// Detache la connexion SI c'est toujours celle-la qui est branchee.
    pub fn detacher_si(&self, numero: u64) {
        let (tampon, _) = &*self.partage;
        let est_lui = {
            let t = tampon.lock().unwrap_or_else(|e| e.into_inner());
            t.abonne.as_ref().is_some_and(|a| a.numero() == numero)
        };
        if est_lui {
            self.detacher();
        }
    }

    /// Detache la connexion (elle est morte, ou une autre prend sa place).
    pub fn detacher(&self) {
        let (tampon, signal) = &*self.partage;
        let mut t = tampon.lock().unwrap_or_else(|e| e.into_inner());
        t.abonne = None;
        t.en_attente.clear();
        signal.notify_all();
    }

    fn abonne(&self) -> Option<Arc<dyn Destinataire>> {
        let (tampon, _) = &*self.partage;
        tampon.lock().unwrap_or_else(|e| e.into_inner()).abonne.clone()
    }

    /// Redemande l'etat complet, historique compris ou non.
    pub fn redessin(&self, avec_historique: bool) -> Vec<u8> {
        let (tampon, _) = &*self.partage;
        let t = tampon.lock().unwrap_or_else(|e| e.into_inner());
        if avec_historique {
            t.ecran.redessiner()
        } else {
            t.ecran.redessiner_ecran()
        }
    }

    pub fn texte_region(&self, debut: (i32, u16), fin: (i32, u16)) -> String {
        let (tampon, _) = &*self.partage;
        tampon.lock().unwrap_or_else(|e| e.into_inner()).ecran.texte_region(debut, fin)
    }

    /// Un geste de recherche. Rend le nombre d'occurrences, l'indice de la courante et sa
    /// position — le frontend n'a plus qu'a defiler et surligner.
    pub fn chercher(
        &self,
        action: super::protocole::ActionRecherche,
        motif: &str,
    ) -> Result<(u32, Option<u32>, Option<crate::terminal::ecran::Occurrence>), String> {
        use super::protocole::ActionRecherche as A;
        let (tampon, _) = &*self.partage;
        let mut t = tampon.lock().unwrap_or_else(|e| e.into_inner());
        match action {
            A::Demarrer => {
                if motif.trim().is_empty() {
                    return Err("recherche vide".into());
                }
                let occurrences = t.ecran.chercher(motif);
                // On demarre par la PLUS RECENTE : c'est ce que fait la loupe d'un
                // terminal, on cherche d'abord ce qui vient de defiler.
                t.recherche.courante = occurrences.len().checked_sub(1);
                t.recherche.motif = motif.to_string();
                t.recherche.occurrences = occurrences;
            }
            A::Suivante | A::Precedente => {
                if t.recherche.occurrences.is_empty() {
                    return Err("aucune recherche en cours".into());
                }
                let dernier = t.recherche.occurrences.len() - 1;
                // « Suivante » remonte vers le plus ancien, comme dans tmux ; les deux
                // sens bouclent plutot que de s'arreter sur un bouton inerte.
                let vers_le_plus_ancien = action == A::Suivante;
                t.recherche.courante = Some(match t.recherche.courante {
                    None => dernier,
                    Some(0) if vers_le_plus_ancien => dernier,
                    Some(i) if vers_le_plus_ancien => i - 1,
                    Some(i) if i == dernier => 0,
                    Some(i) => i + 1,
                });
            }
            A::Annuler => {
                t.recherche = Recherche::default();
            }
        }
        let total = t.recherche.occurrences.len() as u32;
        let index = t.recherche.courante.map(|i| i as u32);
        let occurrence = t.recherche.courante.and_then(|i| t.recherche.occurrences.get(i).copied());
        Ok((total, index, occurrence))
    }

    pub fn vivant(&self) -> bool {
        self.vivant.load(Ordering::SeqCst)
    }

    /// Un agent IA tourne-t-il dans ce terminal ? L'arbre est fourni par l'appelant : il
    /// est construit UNE FOIS par passe de `Lister`, pour tous les terminaux.
    pub fn llm(&self, arbre: &crate::terminal::agents_llm::ArbreProcess) -> bool {
        self.pid.is_some_and(|pid| arbre.contient_un_llm(pid))
    }

    /// Tue le shell. Le thread lecteur constate la fin et previent l'application.
    pub fn fermer(&self) -> Result<(), String> {
        self.tueur.lock().map_err(|_| "terminal deja ferme")?.kill().map_err(|e| e.to_string())
    }
}

/// Boucle du thread lecteur : du PTY vers l'ecran, et rien d'autre.
fn lire_pty(lecteur: &mut (impl Read + ?Sized), session: &Session) {
    let (tampon, signal) = &*session.partage;
    let mut morceau = vec![0u8; LECTURE_PTY];
    loop {
        let lus = match lecteur.read(&mut morceau) {
            Ok(0) | Err(_) => return,
            Ok(n) => n,
        };
        let (sortants, presse_papier, commande, abonne) = {
            let mut t = tampon.lock().unwrap_or_else(|e| e.into_inner());
            t.ecran.avaler(&morceau[..lus]);
            if t.abonne.is_some() {
                t.en_attente.extend_from_slice(&morceau[..lus]);
            }
            // Bascule d'ecran alternatif (vim qui s'ouvre ou se ferme) : le redessin ne
            // rend que l'ecran ACTIF, il faut donc en renvoyer un a chaque bascule, sinon
            // le frontend garde l'ecran de l'application qui vient de se terminer.
            let alternatif = t.ecran.ecran_alternatif();
            if alternatif != t.alternatif {
                t.alternatif = alternatif;
                t.redessin_du = true;
            }
            let mut vers_le_shell = Vec::new();
            let mut vers_le_presse_papier = Vec::new();
            for sortant in t.ecran.sortants() {
                match sortant {
                    Sortant::VersLeShell(texte) => vers_le_shell.push(texte),
                    Sortant::VersLePressePapier(texte) => vers_le_presse_papier.push(texte),
                }
            }
            // Le shell a montre qu'il lit (il vient d'ecrire son invite) : c'est le moment
            // de taper la commande d'ouverture. L'envoyer avant que le shell soit pret la
            // ferait avaler par un `stty` d'initialisation.
            let commande = t.commande_initiale.take();
            (vers_le_shell, vers_le_presse_papier, commande, t.abonne.clone())
        };
        signal.notify_all();

        // Hors du verrou de l'ecran : une reponse au shell ne doit pas attendre une rafale
        // en cours d'ingestion, et un programme qui demande la position du curseur SE FIGE
        // tant qu'il n'a pas sa reponse.
        for texte in sortants {
            if let Err(e) = session.ecrire(texte.as_bytes()) {
                if let Some(abonne) = &abonne {
                    abonne.pousser(Pousse::Panne { id: session.id, message: e });
                }
            }
        }
        if let Some(abonne) = &abonne {
            for texte in presse_papier {
                abonne.pousser(Pousse::PressePapier { id: session.id, texte });
            }
        }
        if let Some(commande) = commande {
            // `\r` et non `\n` : un shell en mode edition de ligne attend un retour
            // chariot, et le PTY en mode canonique le traduit de toute facon.
            if let Err(e) = session.ecrire(format!("{commande}\r").as_bytes()) {
                if let Some(abonne) = &abonne {
                    abonne.pousser(Pousse::Panne { id: session.id, message: e });
                }
            }
        }
    }
}

/// Boucle de l'emetteur : decide, pour chaque lot, s'il part tel quel ou en redessin.
///
/// ## Ce qui declenche le regroupement : le RYTHME, pas le volume
///
/// Une premiere version attendait d'avoir 8 Ko en attente pour regrouper. Elle ne s'est
/// jamais declenchee : un shell ecrit ses lignes au fil de l'eau et le lecteur du PTY est
/// plus rapide que lui, donc chaque lecture rend ~85 octets et chaque lot partait seul.
/// Mesure du 2026-08-21 : `seq 1 200000` (~1,3 Mo) partait en 16 461 envois — c'est-a-dire
/// 16 461 evenements Tauri, chacun une source JavaScript construite et evaluee dans le
/// webview. Le seuil de volume ne pouvait pas voir la rafale ; le rythme, si.
///
/// La regle appliquee, en deux moities :
/// 1. **si le lot precedent etait a peine parti que la suite attendait deja**, c'est une
///    rafale — on laisse `FENETRE_RAFALE` se remplir ;
/// 2. **ou si ce qui attend depasse deja `SEUIL_LOT`**, meme apres attente : sur une
///    machine chargee le shell produit par a-coups, et la premiere moitie seule croit voir
///    des echos la ou il y a une rafale au ralenti.
///
/// Ce qui reste hors des deux : l'echo d'une touche, quelques octets arrives apres un
/// silence. Il part sans delai, et c'est la seule chose qui compte pour la latence.
fn emettre(session: &Session) {
    let (tampon, signal) = &*session.partage;
    loop {
        let (pousse, abonne) = {
            let mut t = tampon.lock().unwrap_or_else(|e| e.into_inner());
            let mut a_attendu = false;
            loop {
                if t.abonne.is_none() {
                    // Personne n'ecoute : on garde l'ecran a jour (le lecteur s'en charge)
                    // mais on ne conserve aucun octet pour un absent.
                    t.en_attente.clear();
                    t.redessin_du = false;
                    t.historique_du = false;
                }
                if t.fini && t.en_attente.is_empty() && !t.redessin_du {
                    return;
                }
                if t.abonne.is_some()
                    && (t.redessin_du || t.historique_du || !t.en_attente.is_empty())
                {
                    break;
                }
                t = signal.wait(t).unwrap_or_else(|e| e.into_inner());
                a_attendu = true;
            }
            let rafale = !a_attendu || t.en_attente.len() >= SEUIL_LOT;
            if rafale && !t.redessin_du && !t.fini {
                // Rafale : on rend le verrou pour que le lecteur continue d'avaler, et on
                // repart avec un lot entier au lieu de quelques dizaines d'octets.
                drop(t);
                std::thread::sleep(FENETRE_RAFALE);
                t = tampon.lock().unwrap_or_else(|e| e.into_inner());
            }
            let Some(abonne) = t.abonne.clone() else { continue };
            if t.redessin_du {
                // Attache, bascule d'ecran alternatif, redimensionnement : AVEC
                // l'historique. Un redessin commence par une remise a plat (RIS) qui vide
                // le tampon de defilement du terminal d'arrivee — sans l'historique,
                // revenir sur un onglet ferait perdre tout ce que la molette remontait.
                t.redessin_du = false;
                t.historique_du = false;
                t.en_attente.clear();
                (Pousse::Redessin { id: session.id, octets: t.ecran.redessiner() }, abonne)
            } else if t.en_attente.len() >= VOLUME_INSOUTENABLE {
                // Debit qu'aucun affichage ne suit : on renonce a transmettre et on decrit
                // l'ecran. L'historique du frontend y passe (RIS), d'ou le drapeau : il lui
                // sera renvoye entier des que le calme reviendra.
                t.en_attente.clear();
                t.historique_du = true;
                (Pousse::Redessin { id: session.id, octets: t.ecran.redessiner_ecran() }, abonne)
            } else if !t.en_attente.is_empty() {
                (
                    Pousse::Sortie { id: session.id, octets: std::mem::take(&mut t.en_attente) },
                    abonne,
                )
            } else {
                // Le calme est revenu apres une rafale insoutenable : on rend au frontend
                // l'historique qu'on lui a fait perdre.
                t.historique_du = false;
                (Pousse::Redessin { id: session.id, octets: t.ecran.redessiner() }, abonne)
            }
        };
        if !abonne.pousser(pousse) {
            session.detacher_si(abonne.numero());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{channel, Receiver, Sender};

    /// Un destinataire en memoire : la session se teste sans socket ni protocole.
    struct Boite(Sender<Pousse>, u64);

    impl Destinataire for Boite {
        fn pousser(&self, pousse: Pousse) -> bool {
            self.0.send(pousse).is_ok()
        }

        fn numero(&self) -> u64 {
            self.1
        }
    }

    fn boite() -> (Arc<dyn Destinataire>, Receiver<Pousse>) {
        static SUIVANT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let (tx, rx) = channel();
        (Arc::new(Boite(tx, SUIVANT.fetch_add(1, Ordering::SeqCst))), rx)
    }

    fn session(commande: Option<&str>) -> Arc<Session> {
        Session::ouvrir(
            1,
            &std::env::temp_dir().to_string_lossy(),
            Taille { colonnes: 80, lignes: 24 },
            commande.map(String::from),
            200,
        )
        .expect("ouverture du shell")
    }

    /// Attend qu'une condition sur l'ecran devienne vraie, ou echoue. Les tests portent sur
    /// un vrai shell : on ne peut pas savoir a l'avance quand il aura repondu.
    fn attendre(session: &Session, quoi: &str, condition: impl Fn(&str) -> bool) -> String {
        let debut = std::time::Instant::now();
        loop {
            let vu = ecran_visible(session);
            if condition(&vu) {
                return vu;
            }
            assert!(
                debut.elapsed() < std::time::Duration::from_secs(20),
                "toujours pas {quoi} apres 20 s. Ecran :\n{vu}"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    fn ecran_visible(session: &Session) -> String {
        session.texte_region((0, 0), (23, 79))
    }

    #[test]
    fn un_shell_repond_a_ce_qu_on_lui_ecrit() {
        let s = session(None);
        s.ecrire(b"echo cockpit-va-bien\r").unwrap();
        attendre(&s, "l'echo du shell", |vu| vu.contains("cockpit-va-bien"));
        s.fermer().unwrap();
    }

    /// La commande d'ouverture (bouton « ▶ Cmd », shell de conteneur, palette).
    #[test]
    fn la_commande_initiale_est_tapee_toute_seule() {
        let s = session(Some("echo depart-automatique"));
        attendre(&s, "la commande initiale", |vu| vu.contains("depart-automatique"));
        s.fermer().unwrap();
    }

    /// L'invariant de la sortie en rafale : le contenu arrive EN ENTIER (c'est lui qui
    /// remplit le tampon de defilement du frontend) mais en gros lots, pas en miettes.
    #[test]
    fn une_rafale_part_en_gros_lots_pas_en_miettes() {
        let s = session(None);
        attendre(&s, "l'invite du shell", |vu| !vu.trim().is_empty());
        let (abonne, recu) = boite();
        s.attacher(abonne);
        // 200 000 lignes numerotees : plusieurs Mo, tres au-dela d'un ecran.
        //
        // Le marqueur de fin est imprime par `printf` en DEUX morceaux : la ligne tapee
        // reste `printf 'rafale%s\n' -finie`, jamais « rafale-finie ». Attendre un
        // marqueur present dans la commande elle-meme rendrait la mesure fausse — le test
        // repartirait avant que le shell ait ecrit un seul octet (constate le 2026-08-21 :
        // il ne mesurait alors que le redessin de l'attache).
        s.ecrire(b"seq 1 200000; printf 'rafale%s\n' -finie\r").unwrap();
        attendre(&s, "la fin de la rafale", |vu| vu.contains("rafale-finie"));
        std::thread::sleep(std::time::Duration::from_millis(200));
        s.fermer().unwrap();

        let mut octets_bruts = 0usize;
        let mut octets_redessin = 0usize;
        let mut nb_bruts = 0usize;
        let mut nb_redessins = 0usize;
        while let Ok(pousse) = recu.recv_timeout(std::time::Duration::from_millis(200)) {
            match pousse {
                Pousse::Sortie { octets, .. } => { octets_bruts += octets.len(); nb_bruts += 1; }
                Pousse::Redessin { octets, .. } => { octets_redessin += octets.len(); nb_redessins += 1; }
                _ => {}
            }
        }
        let total = octets_bruts + octets_redessin;
        let envois = nb_bruts + nb_redessins;
        eprintln!(
            "rafale seq 1..200000 (~1,3 Mo au shell) : {total} octets transmis en {envois} envois \
             ({octets_bruts} bruts en {nb_bruts}, {octets_redessin} de redessins en {nb_redessins})"
        );
        // CE QUI COMPTE EST LE NOMBRE D'ENVOIS, pas le volume : chaque envoi devient un
        // evenement Tauri, c'est-a-dire une source JavaScript construite et evaluee dans le
        // webview. Sans regroupement, cette rafale en faisait 16 461 (mesure du 2026-08-21).
        assert!(
            envois < 600,
            "{envois} envois pour ~1,3 Mo : le regroupement ne fait plus son travail"
        );
        // Et le contenu, lui, arrive VRAIMENT : c'est ce qui remplit le tampon de
        // defilement du terminal du frontend, donc ce qui fait marcher la molette.
        assert!(
            total > 900 * 1024,
            "{total} octets seulement : la rafale n'a pas ete transmise"
        );
    }

    /// Le pendant du test precedent : ce qui est petit ne doit PAS declencher de redessin,
    /// sinon chaque frappe ferait repeindre tout l'ecran.
    #[test]
    fn un_echo_de_touche_part_tel_quel() {
        let s = session(None);
        attendre(&s, "l'invite du shell", |vu| !vu.trim().is_empty());
        let (abonne, recu) = boite();
        s.attacher(abonne);
        // Le redessin de l'attache.
        assert!(matches!(
            recu.recv_timeout(std::time::Duration::from_secs(5)),
            Ok(Pousse::Redessin { .. })
        ));
        s.ecrire(b"x").unwrap();
        let suite = recu.recv_timeout(std::time::Duration::from_secs(5)).expect("echo");
        match suite {
            Pousse::Sortie { octets, .. } => {
                assert!(octets.len() < 64, "echo de {} octets : {octets:?}", octets.len())
            }
            autre => panic!("l'echo d'une touche devrait partir tel quel, recu {autre:?}"),
        }
        s.fermer().unwrap();
    }

    /// La promesse du produit : le client s'en va, le shell continue, on revient et
    /// l'ecran est le meme.
    #[test]
    fn l_ecran_survit_au_depart_du_client() {
        let s = session(None);
        s.ecrire(b"echo marqueur-de-survie\r").unwrap();
        attendre(&s, "le marqueur", |vu| vu.contains("marqueur-de-survie"));

        let (abonne, recu) = boite();
        s.attacher(abonne);
        let premier = match recu.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
            Pousse::Redessin { octets, .. } => octets,
            autre => panic!("attendu un redessin, recu {autre:?}"),
        };
        drop(recu);
        s.detacher();

        // Le shell continue de tourner pendant que personne n'ecoute.
        s.ecrire(b"echo pendant-l-absence\r").unwrap();
        attendre(&s, "la ligne ecrite en l'absence du client", |vu| {
            vu.contains("pendant-l-absence")
        });

        let (abonne2, recu2) = boite();
        s.attacher(abonne2);
        let second = match recu2.recv_timeout(std::time::Duration::from_secs(5)).unwrap() {
            Pousse::Redessin { octets, .. } => octets,
            autre => panic!("attendu un redessin, recu {autre:?}"),
        };
        assert!(!premier.is_empty() && !second.is_empty());
        // Le second redessin contient les deux lignes : rien n'a ete perdu en l'absence.
        let texte = String::from_utf8_lossy(&second);
        assert!(texte.contains("marqueur-de-survie"), "{texte}");
        assert!(texte.contains("pendant-l-absence"), "{texte}");
        s.fermer().unwrap();
    }

    /// La recherche, y compris dans ce qui a defile hors de l'ecran.
    #[test]
    fn la_recherche_trouve_dans_l_historique() {
        use super::super::protocole::ActionRecherche;
        let s = session(None);
        // Le marqueur ne doit apparaitre QUE dans la sortie : `printf 'aiguille%s'`
        // laisse « aiguille%s » sur la ligne tapee, jamais « aiguille-rare ».
        s.ecrire(b"printf 'aiguille%s\\n' -rare\r").unwrap();
        attendre(&s, "le marqueur", |vu| vu.contains("aiguille-rare"));
        s.ecrire(b"seq 1 100\r").unwrap();
        attendre(&s, "la fin du seq", |vu| vu.contains("100") && !vu.contains("aiguille-rare"));

        let (total, index, ou) = s.chercher(ActionRecherche::Demarrer, "aiguille-rare").unwrap();
        assert!(total >= 1, "total={total}");
        assert_eq!(index, Some(total - 1));
        // Elle a defile hors de l'ecran : sa ligne est negative.
        assert!(ou.unwrap().ligne < 0, "{ou:?}");

        // Un motif vide est une ERREUR remontee, pas un silence.
        assert!(s.chercher(ActionRecherche::Demarrer, "   ").is_err());
        s.fermer().unwrap();
    }


}
