//! Un terminal, cote service : un shell dans un PTY, et l'ecran qu'il dessine.
//!
//! ## Ce qui part vers l'application : le flux, mais par lots
//!
//! Tout ce que le shell ecrit est transmis — c'est ce qui remplit le tampon de defilement
//! du terminal du frontend, donc ce qui fait marcher la molette sans rien demander a
//! personne. Ce qui est REGROUPE, c'est le nombre d'envois : chacun devient un evenement
//! Tauri, c'est-a-dire une source JavaScript construite et evaluee dans le webview.
//!
//! La regle appliquee ici (voir `emettre` pour le detail) :
//! - **un petit lot arrive apres un silence part TEL QUEL, tout de suite** : c'est l'echo
//!   d'une touche, il ne doit rien attendre ;
//! - **le reste attend `FENETRE_RAFALE`** pour partir groupe ;
//! - **un lot au-dela de `VOLUME_INSOUTENABLE` est REMPLACE par un redessin** : a 32 Mo/s
//!   personne ne lit ce qui defile, le contenu a forcement ete recouvert, et le redessin
//!   est plus court ET plus juste. L'historique complet est renvoye des que le calme
//!   revient, pour rendre au frontend ce que la remise a plat lui a fait perdre.
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

/// Au-dela de ce volume, un lot n'est plus l'echo d'une frappe.
///
/// Sert a distinguer les deux choses qui arrivent en cadence soutenue : une rafale de
/// sortie, et une frappe rapide (ou une touche maintenue). L'echo d'une touche fait
/// quelques octets — le caractere rendu par le terminal, parfois double par le programme
/// qui le relit. Un lot de rafale fait au moins quelques dizaines d'octets : 85 en moyenne
/// sous Linux, ~295 sur le runner macOS. 64 octets passe entre les deux avec de la marge
/// des deux cotes.
///
/// NE PAS monter cette valeur pour « mieux regrouper » : ce qui la depasse et arrive en
/// cadence prend jusqu'a 8 ms de retard, et une frappe ne doit jamais en prendre.
const TAILLE_ECHO: usize = 64;

/// Combien de temps on attend qu'un shell tue rende vraiment la main.
///
/// La fermeture se CONSTATE (voir `Session::fermer`) : ce delai est la seule chose qui
/// distingue « c'est parti » de « ca ne part pas ». Il ne cadence rien — un shell tue rend
/// la main en quelques millisecondes — il ne sert qu'a ne pas attendre indefiniment.
const DELAI_FERMETURE: std::time::Duration = std::time::Duration::from_secs(2);

/// Ce qu'on laisse au thread lecteur pour finir d'avaler la sortie avant de fermer le PTY.
///
/// Sous Unix il a deja fini (la mort du shell lui a donne sa fin de fichier) et ce delai ne
/// change rien. Sous Windows c'est la fermeture qui le debloque, et fermer trop tot lui
/// couperait les derniers octets ecrits par le programme qui s'arrete.
const GRACE_DE_LECTURE: std::time::Duration = std::time::Duration::from_millis(300);

/// Volume d'un seul lot au-dela duquel on renonce a transmettre et on redessine.
///
/// C'est un PLAFOND DE MEMOIRE — ce qu'on accepte de garder pour un frontend — et non un
/// jugement sur le debit. La nuance a coute une regression : la valeur etait de 256 Ko,
/// justifiee par « 256 Ko dans une fenetre de 8 ms, c'est 32 Mo/s, aucun affichage ne
/// suit ». Ce raisonnement est faux. Un gros lot ne dit pas que le debit est ingerable, il
/// dit que le PTY a livre son tampon d'un coup — ce que macOS fait beaucoup plus qu'Linux.
/// Mesure du runner macOS de la v0.39.0 : sur 1,3 Mo de `seq 1 200000`, une sortie tout a
/// fait ordinaire, **368 Ko seulement arrivaient** et le reste etait remplace par sept
/// redessins. Concretement l'utilisateur remontait a la molette et ne trouvait pas sa
/// sortie : c'est precisement ce que le flux brut existe pour eviter.
///
/// 4 Mo, donc : au-dessus de toute sortie de commande normale (un gros journal de build fait
/// quelques Mo), et ca reste borne — c'est la seule chose qui compte pour un flux sans fin
/// (`cat /dev/urandom`), ou la protection garde tout son sens. En dessous on transmet TOUT,
/// et c'est ce qui remplit le tampon de defilement du terminal du frontend.
const VOLUME_INSOUTENABLE: usize = 4 * 1024 * 1024;

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
    /// `None` des que le shell est mort : le relacher FERME le pseudo-terminal, ce qui est
    /// la seule facon de debloquer le lecteur sous Windows (voir `guetter_la_fin`).
    maitre: Mutex<Option<Box<dyn MasterPty + Send>>>,
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
            maitre: Mutex::new(Some(paire.master)),
            tueur: Mutex::new(tueur),
            pid,
            vivant: Arc::new(AtomicBool::new(true)),
            partage: Arc::clone(&partage),
            taille: Mutex::new(taille),
        });

        {
            let session_guetteur = Arc::clone(&session);
            std::thread::spawn(move || {
                let mut enfant = enfant;
                // Bloque jusqu'a la mort du shell, et le RAMASSE du meme coup : sans ca il
                // resterait zombie tant que le service tourne, et le service tourne des jours.
                let _ = enfant.wait();
                session_guetteur.guetter_la_fin();
            });
        }

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

    /// Le shell vient de mourir : on le dit, et on debloque le lecteur.
    ///
    /// ## Pourquoi le PROCESS est l'autorite, et pas le tuyau
    ///
    /// Le thread lecteur constatait la fin en recevant la fin de fichier du PTY. C'est vrai
    /// sous Unix — la mort du shell ferme l'esclave — et FAUX sous Windows : ConPTY garde son
    /// tuyau ouvert apres la mort du shell (c'est `conhost` qui le tient, pas le shell), donc
    /// la lecture ne rend jamais rien. Consequences mesurees sur le runner le 2026-08-21 :
    /// `vivant` restait vrai indefiniment, donc la fin d'un terminal n'etait JAMAIS annoncee
    /// a l'application, la session ne se refermait pas cote service, et `fermer()` attendait
    /// pour rien. Attendre le PROCESS, lui, marche partout.
    ///
    /// Relacher le maitre ferme le pseudo-terminal, ce qui debloque le lecteur. On laisse
    /// d'abord passer `GRACE_DE_LECTURE` : sous Unix le lecteur a deja fini d'avaler ce qui
    /// restait (il a eu sa fin de fichier), et fermer trop tot lui couperait les derniers
    /// octets — ceux qui portent souvent le message d'adieu d'un programme.
    fn guetter_la_fin(&self) {
        self.vivant.store(false, Ordering::SeqCst);
        {
            let (tampon, signal) = &*self.partage;
            tampon.lock().unwrap_or_else(|e| e.into_inner()).fini = true;
            signal.notify_all();
        }
        std::thread::sleep(GRACE_DE_LECTURE);
        // Le `drop` a lieu HORS du verrou : fermer un pseudo-terminal peut bloquer, et le
        // garder pris pendant ce temps figerait un redimensionnement concurrent.
        let maitre = self.maitre.lock().map(|mut m| m.take()).unwrap_or(None);
        drop(maitre);
    }

    /// Redimensionne le PTY. Sans effet — et sans erreur — quand le shell est mort : le
    /// frontend continue de mesurer son conteneur pendant qu'un onglet se referme, et ce
    /// n'est pas une panne a montrer a l'utilisateur.
    pub fn redimensionner(&self, taille: Taille) -> Result<(), String> {
        if let Some(maitre) = self.maitre.lock().map_err(|_| "PTY perdu")?.as_mut() {
            maitre
                .resize(PtySize {
                    rows: taille.lignes.max(1),
                    cols: taille.colonnes.max(1),
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| e.to_string())?;
        }
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

    /// Tue le shell, et rend la main quand il est VRAIMENT parti.
    ///
    /// ## Pourquoi on ne croit pas le code de retour de `kill()`
    ///
    /// `portable-pty` 0.9.0 a le test inverse dans `WinChildKiller::kill`
    /// (`src/win/mod.rs`) :
    ///
    /// ```text
    /// let res = unsafe { TerminateProcess(...) };
    /// let err = IoError::last_os_error();
    /// if res != 0 { Err(err) } else { Ok(()) }
    /// ```
    ///
    /// `TerminateProcess` rend NON-ZERO en cas de succes. Donc un kill qui a marche remonte
    /// `Err(last_os_error())` — soit « The operation completed successfully. (os error 0) »
    /// quand rien n'a pose de code, soit une erreur PERIMEE d'un appel anterieur du meme
    /// thread, d'ou des « The handle is invalid. (os error 6) » incomprehensibles. Et un kill
    /// qui a echoue remonte `Ok(())`. Le code de retour ne veut donc rien dire du tout sous
    /// Windows. (`WinChild::kill` avale le sien par `.ok()` ; c'est le killer CLONE, celui
    /// qu'on utilise, qui le propage.)
    ///
    /// ## Ce qu'on fait a la place
    ///
    /// On constate. `fermer` veut dire « le shell est parti », c'est donc ca qu'on verifie :
    /// le thread lecteur passe `vivant` a faux des que le PTY rend la fin de fichier. Une
    /// seule regle pour les trois systemes, et elle ne depend d'aucune bibliotheque.
    ///
    /// Cas ordinaire couvert au passage : un shell DEJA termine (on tape `exit`, puis on
    /// ferme l'onglet). `vivant` est deja faux, on rend `Ok` sans rien attendre — avant, ce
    /// geste parfaitement normal remontait une erreur a l'utilisateur.
    pub fn fermer(&self) -> Result<(), String> {
        self.tueur.lock().map_err(|_| "terminal deja ferme")?.kill().ok();
        // On laisse d'abord le shell partir de lui-meme : il en profite pour ecrire son
        // historique. Le cas ordinaire tient en 10 a 25 ms, mesure localement.
        if self.attendre_la_fin(DELAI_FERMETURE) {
            return Ok(());
        }
        self.insister();
        if self.attendre_la_fin(DELAI_FERMETURE) {
            return Ok(());
        }
        Err(format!(
            "le shell n'a pas rendu la main en {} s",
            DELAI_FERMETURE.as_secs() * 2
        ))
    }

    /// Attend que le shell soit parti, au plus `delai`. Rend `true` s'il est parti.
    fn attendre_la_fin(&self, delai: std::time::Duration) -> bool {
        let debut = std::time::Instant::now();
        while self.vivant() && debut.elapsed() < delai {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        !self.vivant()
    }

    /// Le coup de grace, quand la demande polie est restee sans effet.
    ///
    /// `portable-pty` envoie un SIGHUP sous Unix, et son propre commentaire le dit : le
    /// processus est tue « SAUF s'il a installe un gestionnaire ». Un shell a parfaitement le
    /// droit de l'ignorer (`trap "" HUP`, et certains programmes plein ecran le font). Sans
    /// cette insistance, fermer un onglet rendait une erreur a l'utilisateur ET laissait le
    /// process tourner — dans un service qui, lui, tourne des jours.
    #[cfg(unix)]
    fn insister(&self) {
        if let Some(pid) = self.pid {
            // SIGKILL ne se refuse pas. On ne regarde pas le retour : un echec peut vouloir
            // dire « deja parti », ce qui est exactement le resultat cherche. Comme partout
            // dans ce fichier, c'est `vivant` qui tranche, pas un code de retour.
            unsafe { libc::kill(pid as i32, libc::SIGKILL) };
        }
    }

    /// Sous Windows, le premier coup est deja un `TerminateProcess` : il ne se refuse pas, et
    /// il n'existe pas de signal a envoyer par-dessus.
    #[cfg(not(unix))]
    fn insister(&self) {}
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
/// 1. **si le lot precedent est parti il y a moins de `FENETRE_RAFALE` ET qu'il depassait
///    `TAILLE_ECHO`**, c'est une rafale — on laisse la fenetre se remplir au lieu d'envoyer
///    des miettes. Les deux conditions comptent : une frappe rapide, elle aussi, tient une
///    cadence soutenue, et elle ne doit jamais attendre. C'est la TAILLE qui les separe (un
///    echo fait quelques octets) ;
/// 2. **ou si ce qui attend depasse deja `SEUIL_LOT`** : sur une machine chargee le shell
///    produit par a-coups, et la premiere moitie seule croit voir des echos la ou il y a
///    une rafale au ralenti.
///
/// Ce qui reste hors des deux : l'echo d'une touche, quelques octets arrives apres un
/// silence. Il part sans delai, et c'est la seule chose qui compte pour la latence.
///
/// ## Pourquoi on mesure la CADENCE et non « il a fallu attendre »
///
/// La premiere moitie a d'abord ete ecrite « si la suite attendait deja quand on est
/// revenu » (`!a_attendu`). Ca marche seulement quand le lecteur du PTY va plus vite que
/// l'emetteur — vrai sous Linux, FAUX sous macOS, ou chaque reveil trouvait environ 295
/// octets et repartait aussitot : la rafale n'etait jamais reconnue. Mesure du runner
/// macOS de la v0.38.0 : **3 047 envois** pour 0,9 Mo, la ou Linux en fait quelques
/// dizaines. Et le meme defaut expliquait la deuxieme moitie du symptome : un emetteur qui
/// part 3 047 fois draine trop lentement, donc la fermeture de la session jetait ~400 Ko
/// jamais transmis.
///
/// « Le lot precedent est-il parti il y a moins de 8 ms » ne depend, lui, ni du systeme ni
/// de l'ordonnancement de deux threads : c'est le debit reel de nos propres envois. NE PAS
/// revenir a `!a_attendu` — il ne mesure pas ce qu'il a l'air de mesurer.
///
/// Piege paye au passage : la cadence SEULE prend une frappe rapide pour une rafale. L'essai
/// de latence fait 200 allers-retours a la suite, donc en cadence soutenue par construction,
/// et le surcout du service est passe de 0,06 ms a 8,5 ms — soit exactement la surcouche que
/// ce projet interdit sur le chemin de frappe. D'ou `TAILLE_ECHO`.
///
/// ## Et pourquoi c'est la taille du lot PRECEDENT qui compte
///
/// La taille regardee a d'abord ete celle de ce qui attend A L'INSTANT de la decision. Ca
/// marche sur une machine rapide et pas sur une machine lente : le thread lecteur tient le
/// verrou pendant qu'il fait avaler les octets a l'ecran, donc sur deux coeurs charges
/// l'emetteur l'obtient rarement et avec peu de choses dedans — il conclut « ce n'est pas une
/// rafale » et repart aussitot, ce qui refabrique exactement le defaut. Mesure du runner Linux
/// de la v0.41.2 : **2 810 envois pour 1,5 Mo, soit 529 octets chacun**, la ou cette machine
/// en fait 70 a 20 Ko.
///
/// La taille du lot PRECEDENT ne depend pas de cet alea : elle dit ce que le shell produit
/// vraiment. Au pire un seul petit envoi part au debut d'une rafale, et la regle est accrochee
/// des le suivant.
fn emettre(session: &Session) {
    let (tampon, signal) = &*session.partage;
    // Quand le lot precedent est parti. `None` tant qu'on n'a rien envoye : le premier lot
    // ne doit pas etre retenu.
    let mut derniere_emission: Option<std::time::Instant> = None;
    // Et sa TAILLE. C'est elle qui dit si on est dans une rafale ou sous une frappe, parce
    // qu'elle ne depend pas du moment ou l'emetteur obtient le verrou (voir plus bas).
    let mut derniere_taille = 0usize;
    loop {
        let (pousse, abonne) = {
            let mut t = tampon.lock().unwrap_or_else(|e| e.into_inner());
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
            }
            let cadence_soutenue =
                derniere_emission.is_some_and(|parti| parti.elapsed() < FENETRE_RAFALE);
            let rafale = t.en_attente.len() >= SEUIL_LOT
                || (cadence_soutenue && derniere_taille > TAILLE_ECHO);
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
        derniere_emission = Some(std::time::Instant::now());
        // Un REDESSIN remet la taille a zero : il arrive a l'attache, et la premiere touche
        // tapee juste apres ne doit pas etre retenue sous pretexte que le redessin etait gros.
        derniere_taille = match &pousse {
            Pousse::Sortie { octets, .. } => octets.len(),
            _ => 0,
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

    /// Un pid existe-t-il encore ? Le signal 0 ne fait rien : il ne sert qu'a repondre a cette
    /// question. Le pere de l'enfant est mort, donc l'orphelin est adopte et ramasse — pas de
    /// zombie qui repondrait « vivant » alors qu'il ne tourne plus.
    #[cfg(unix)]
    fn pid_vivant(pid: i32) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    /// Le filet de l'essai : un `assert!` rate ne doit pas laisser un dormeur derriere lui.
    /// Sans lui, chaque echec ajoute un processus invisible sur la machine — la falsification
    /// de cet essai en a laisse un.
    #[cfg(unix)]
    struct Faucheuse(i32);

    #[cfg(unix)]
    impl Drop for Faucheuse {
        fn drop(&mut self) {
            if pid_vivant(self.0) {
                unsafe { libc::kill(self.0, libc::SIGKILL) };
            }
        }
    }

    /// LA QUESTION : fermer un onglet emporte-t-il ce qui TOURNE dedans ?
    ///
    /// `claude` n'est pas le shell, c'est son enfant au premier plan. Le coup de grace ne vise
    /// que le shell — `kill(pid)` et non `kill(-pid)` — donc l'enfant ne meurt pas de la. Ce qui
    /// l'emporte est le RACCROCHAGE du pseudo-terminal : quand le maitre est relache, le noyau
    /// envoie SIGHUP au groupe de premier plan de la session.
    ///
    /// Cet enchainement se MESURE, il ne se deduit pas : il depend de qui relache le maitre et
    /// quand. Un enfant qui survivrait tournerait des jours dans un service qui, lui, ne
    /// s'arrete pas — et personne ne le verrait.
    #[test]
    #[cfg(unix)]
    fn fermer_un_onglet_emporte_ce_qui_tourne_dedans() {
        let s = session(None);

        // `exec` remplace le sous-shell : le pid annonce est donc bien celui du dormeur, et il
        // est au premier plan, comme claude. Le marqueur est coupe en deux (`enf""ant`) pour
        // que le shell le RECONSTRUISE : sinon on retrouve d'abord l'echo de ce qu'on tape.
        s.ecrire(b"sh -c 'echo enf\"\"ant=$$; exec sleep 600'\r").unwrap();
        let vu = attendre(&s, "le pid de l'enfant", |v| v.contains("enfant="));

        let pid: i32 = vu
            .split("enfant=")
            .nth(1)
            .and_then(|reste| {
                let chiffres: String = reste.chars().take_while(|c| c.is_ascii_digit()).collect();
                chiffres.parse().ok()
            })
            .unwrap_or_else(|| panic!("pid de l'enfant introuvable. Ecran :\n{vu}"));

        let _faucheuse = Faucheuse(pid);

        // Sans cette verification, l'essai passerait aussi quand l'enfant n'a jamais demarre :
        // il chercherait alors une panne qui n'existe pas.
        assert!(pid_vivant(pid), "l'enfant ({pid}) doit tourner avant qu'on ferme");

        s.fermer().unwrap();

        // Le raccrochage se propage de facon asynchrone : on laisse au noyau le temps de
        // signaler le groupe de premier plan, sans figer l'essai si c'est deja fait.
        let debut = std::time::Instant::now();
        while pid_vivant(pid) && debut.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        assert!(
            !pid_vivant(pid),
            "l'enfant ({pid}) tourne encore apres la fermeture de l'onglet : \
             fermer un terminal ou claude tourne laisserait claude en vie",
        );
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
    ///
    /// Reserve aux shells POSIX : la ligne tapee enchaine deux commandes par `;`, et
    /// `cmd.exe` ne connait pas ce separateur (il utilise `&`). Sur le runner Windows, toute
    /// la ligne partait donc en arguments de `seq`, qui repondait « extra operand ». Ne PAS
    /// bricoler un equivalent `cmd.exe` sans machine pour l'essayer : le marqueur de fin ne
    /// doit pas figurer dans la ligne TAPEE (le PTY en renvoie l'echo avant execution), et
    /// les tournures qui permettent ca en `cmd.exe` sont exactement le genre de chose qu'on
    /// ne peut pas valider a l'aveugle. Le regroupement lui-meme n'a rien de specifique a un
    /// systeme — il est verifie ici, et macOS le verifie aussi.
    #[cfg(unix)]
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
        let ecran = attendre(&s, "la fin de la rafale", |vu| vu.contains("rafale-finie"));
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
        let lot_moyen = total / envois.max(1);
        eprintln!(
            "rafale seq 1..200000 (~1,3 Mo au shell) : {total} octets transmis en {envois} envois, \
             soit {lot_moyen} octets par envoi \
             ({octets_bruts} bruts en {nb_bruts}, {octets_redessin} de redessins en {nb_redessins})"
        );
        // CE QUI COMPTE EST LA TAILLE MOYENNE D'UN ENVOI, et non leur nombre. Chaque envoi
        // devient un evenement Tauri, c'est-a-dire une source JavaScript construite et evaluee
        // dans le webview : ce qu'on veut garantir, c'est qu'un evenement porte un vrai lot et
        // pas quelques octets.
        //
        // Le NOMBRE ne peut pas servir de borne, et c'est une lecon payee : un lot part au
        // plus toutes les 8 ms, donc le nombre d'envois suit la DUREE de la rafale. Sur cette
        // machine `seq 1 200000` prend une fraction de seconde (19 a 123 envois selon la
        // charge) ; sur un runner lent la meme rafale s'etale et depasse allegrement les 600
        // envois qui etaient ecrits ici — sans qu'aucun regroupement n'ait cesse de marcher.
        // La borne d'origine mesurait la vitesse de la machine, pas le regroupement.
        //
        // Ordres de grandeur du DEFAUT, eux, sont sans ambiguite : 16 461 envois pour 1,3 Mo
        // sans aucun regroupement (~85 octets par envoi), et 3 047 sur le runner macOS quand
        // la regle ne se declenchait jamais (~295 octets). Un seuil a 1 Ko de moyenne les
        // attrape tous les deux et laisse passer toute machine, quelle que soit sa vitesse.
        // CETTE ASSERTION VIENT EN PREMIER, ET C'EST DELIBERE. Le contenu doit arriver
        // VRAIMENT — c'est lui qui remplit le tampon de defilement du frontend, donc ce qui
        // fait marcher la molette. Mais surtout : si le shell n'a pas produit sa rafale (il
        // arrive qu'il n'ait pas pu lancer `seq`, machine chargee), la moyenne se calcule sur
        // quelques centaines d'octets et la borne suivante accuse le regroupement d'une panne
        // qui n'existe pas. Constate : « 24 octets par envoi : le regroupement ne fait plus
        // son travail » pour 897 octets transmis en tout. Un essai doit designer la BONNE
        // cause, sinon il envoie chercher ailleurs.
        assert!(
            total > 900 * 1024,
            "{total} octets seulement : la rafale n'a pas eu lieu, il n'y a rien a mesurer. \
             Dernieres lignes de l'ecran :\n{}",
            // Les lignes VIDES sont ecartees : sous le curseur la grille est blanche, et
            // prendre betement la fin ne montrait rien du tout.
            ecran
                .lines()
                .filter(|l| !l.trim().is_empty())
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(
            lot_moyen > 1024,
            "{lot_moyen} octets par envoi ({envois} envois pour {total} octets) : \
             le regroupement ne fait plus son travail"
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
        // Vider ce que le shell a dit en demarrant — son invite, et sous Windows le titre de
        // fenetre par-dessus. Sans ca on mesure l'invite au lieu de l'echo : `cmd.exe`
        // reaffiche `C:\Users\...\Temp>` plus une sequence de titre, soit 87 octets, et
        // l'essai concluait que l'echo d'une touche etait gros (constate sur le runner
        // Windows de la v0.41.1). Attendre le silence est aussi plus juste sous Unix, ou la
        // meme course existait, simplement plus etroite.
        while recu.recv_timeout(std::time::Duration::from_millis(300)).is_ok() {}
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



    /// Fermer un terminal dont le programme s'est deja arrete doit REUSSIR.
    ///
    /// `exit` est la seule commande de ce fichier qui vaille sur les trois systemes (les
    /// shells POSIX et `cmd.exe` la connaissent), donc cet essai n'a pas besoin de garde.
    /// Fermer un onglet doit fermer le terminal, meme quand le shell refuse de partir.
    ///
    /// `portable-pty` envoie un SIGHUP, et son propre commentaire le dit : « le processus est
    /// tue SAUF s'il a installe un gestionnaire ». Un shell qui l'ignore survivait donc a la
    /// fermeture de son onglet : l'utilisateur voyait une erreur, et le processus restait —
    /// dans un service qui tourne des jours.
    ///
    /// Reserve a Unix : le SIGHUP n'existe pas ailleurs, et sous Windows `TerminateProcess`
    /// ne se refuse pas.
    #[cfg(unix)]
    #[test]
    fn un_shell_qui_ignore_le_signal_est_ferme_quand_meme() {
        // Le marqueur est CONSTRUIT par le shell : tel quel, on le trouverait dans l'echo de
        // la frappe, avant meme que le `trap` soit pose.
        let s = session(Some("trap \"\" HUP; echo sour\"d\"-au-signal"));
        attendre(&s, "le shell devenu sourd", |vu| vu.contains("sourd-au-signal"));
        let pid = s.pid.expect("le shell a un pid");
        s.fermer().expect("fermer un shell qui ignore le signal");
        assert!(!s.vivant(), "la session se croit encore vivante");
        // On CONSTATE la disparition du process, on ne croit pas un code de retour.
        let debut = std::time::Instant::now();
        while std::path::Path::new(&format!("/proc/{pid}")).exists()
            && debut.elapsed() < std::time::Duration::from_secs(5)
        {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            !std::path::Path::new(&format!("/proc/{pid}")).exists(),
            "le shell {pid} tourne toujours apres fermer()"
        );
    }

    #[test]
    fn fermer_un_shell_deja_termine_n_est_pas_une_erreur() {
        let s = session(None);
        attendre(&s, "l'invite du shell", |vu| !vu.trim().is_empty());
        s.ecrire(b"exit\r").unwrap();
        let debut = std::time::Instant::now();
        while s.vivant() && debut.elapsed() < std::time::Duration::from_secs(20) {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!s.vivant(), "le shell devait s'arreter apres `exit`");
        s.fermer().expect("fermer un terminal deja termine doit reussir");
    }
}
