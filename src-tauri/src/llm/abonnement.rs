//! L'etat de la connexion a un fournisseur, et la connexion guidee par son CLI.
//!
//! Certains fournisseurs se paient a l'abonnement et gardent leurs jetons eux-memes, dans un
//! fichier a eux : on ne fait que LIRE cet etat, on ne detient rien. D'autres n'ont que des
//! cles d'API, et n'implementent donc pas ce trait — l'ecran des reglages ne leur propose alors
//! aucune connexion, au lieu d'un bouton qui ne mene nulle part.
//!
//! La connexion guidee, elle, est la meme mecanique pour tous : on lance une sous-commande de
//! leur CLI dans un pseudo-terminal, la sortie part au frontend telle quelle, et on renvoie ce
//! que la personne recopie. **Ce que le CLI affiche n'est pas interprete** : c'est lui qui parle
//! a son service et qui range ses jetons, et une analyse de sa sortie casserait a sa prochaine
//! mise a jour.

use crate::commande::SansConsole;
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

/// L'evenement qui porte la sortie du CLI pendant une connexion guidee.
pub const EVENEMENT_SORTIE: &str = "llm_connexion_sortie";
/// Et celui qui dit que c'est fini, quelle qu'en soit la raison.
pub const EVENEMENT_FIN: &str = "llm_connexion_fin";

/// Ce qu'un fournisseur sait dire de la connexion, avant qu'on y ajoute son identite.
#[derive(Default, Clone)]
pub struct Etat {
    pub connecte: bool,
    /// La formule souscrite, telle que le fournisseur la nomme.
    pub formule: Option<String>,
    /// Le palier de limitation, quand il en annonce un.
    pub palier: Option<String>,
    /// Epoch secondes d'expiration du jeton courant.
    pub expire_le: Option<i64>,
    /// Pourquoi l'etat n'a pas pu etre determine, quand c'est le cas.
    ///
    /// **« Non connecte » et « on n'a pas su regarder » sont deux choses differentes**, et
    /// elles s'affichaient pareil : un dossier personnel introuvable ou un fichier de jetons
    /// illisible rendaient « pas connecte », sans un mot. On relancait alors une connexion qui
    /// ne changeait rien.
    pub probleme: Option<String>,
}

/// L'etat complet, tel qu'il part au frontend.
#[derive(Serialize, Clone)]
pub struct EtatAbonnement {
    pub fournisseur: String,
    pub nom: String,
    /// Le fournisseur sait-il seulement parler d'abonnement ?
    pub gere_abonnement: bool,
    pub cli_installe: bool,
    pub cli_version: Option<String>,
    /// La sous-commande qui ouvre la connexion guidee, quand il en propose une.
    pub connexion_guidee: bool,
    pub connecte: bool,
    pub formule: Option<String>,
    pub palier: Option<String>,
    pub expire_le: Option<i64>,
    pub probleme: Option<String>,
}

/// La commande a lancer pour une connexion guidee.
pub struct ConnexionGuidee {
    pub programme: &'static str,
    pub arguments: &'static [&'static str],
}

pub trait Abonnement: Send + Sync {
    /// Ce qu'on peut lire de l'etat de connexion, sans rien lancer.
    fn etat(&self) -> Etat;

    /// La sous-commande de son CLI qui ouvre une connexion guidee.
    fn connexion_guidee(&self) -> Option<ConnexionGuidee> {
        None
    }

    /// La version du CLI, quand il sait la dire. **Un lancement de processus**, donc jamais
    /// dans une boucle : seul l'ecran des reglages le demande, pour le fournisseur affiche.
    fn version_cli(&self) -> Option<String> {
        None
    }
}

/// L'etat de connexion du fournisseur donne.
pub fn etat(fournisseur: &'static dyn super::Fournisseur) -> EtatAbonnement {
    let cli_installe = fournisseur.commandes().iter().any(|c| super::dans_le_chemin(c));
    let mut complet = EtatAbonnement {
        fournisseur: fournisseur.id().to_string(),
        nom: fournisseur.nom().to_string(),
        gere_abonnement: fournisseur.abonnement().is_some(),
        cli_installe,
        cli_version: None,
        connexion_guidee: false,
        connecte: false,
        formule: None,
        palier: None,
        expire_le: None,
        probleme: None,
    };
    let Some(abonnement) = fournisseur.abonnement() else {
        return complet;
    };
    complet.connexion_guidee = abonnement.connexion_guidee().is_some();
    // La version ne se demande que si le CLI est la : sinon c'est un lancement de processus
    // pour rien, et sous Windows une console qui clignote pour rien.
    if cli_installe {
        complet.cli_version = abonnement.version_cli();
    }
    let lu = abonnement.etat();
    complet.connecte = lu.connecte;
    complet.formule = lu.formule;
    complet.palier = lu.palier;
    complet.expire_le = lu.expire_le;
    complet.probleme = lu.probleme;
    complet
}

/// Demande sa version a un CLI. Utilitaire pour les implementations.
pub fn version_par_cli(programme: &str) -> Option<String> {
    std::process::Command::new(programme)
        .sans_console()
        .arg("--version")
        .output()
        .ok()
        .filter(|sortie| sortie.status.success())
        .map(|sortie| String::from_utf8_lossy(&sortie.stdout).trim().to_string())
}

/// La connexion guidee en cours. Une seule a la fois : deux CLI qui attendent un code en meme
/// temps, c'est un code colle au mauvais.
#[derive(Default)]
pub struct SessionConnexion {
    session: Mutex<Option<EnCours>>,
}

struct EnCours {
    entree: Box<dyn Write + Send>,
    tueur: Box<dyn ChildKiller + Send + Sync>,
    vivant: Arc<std::sync::atomic::AtomicBool>,
    /// Garde le pseudo-terminal ouvert pendant tout l'echange.
    _maitre: Box<dyn MasterPty + Send>,
}

impl SessionConnexion {
    /// Lance la connexion guidee du fournisseur : sa sortie part par `EVENEMENT_SORTIE`, la fin
    /// par `EVENEMENT_FIN`.
    pub fn demarrer(
        &self,
        app: AppHandle,
        fournisseur: &'static dyn super::Fournisseur,
    ) -> Result<(), String> {
        let guidee = fournisseur
            .abonnement()
            .and_then(|a| a.connexion_guidee())
            .ok_or_else(|| format!("{} ne propose pas de connexion guidee", fournisseur.nom()))?;

        self.annuler();

        let systeme = native_pty_system();
        let paire = systeme
            .openpty(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| format!("openpty: {e}"))?;

        let mut commande = CommandBuilder::new(guidee.programme);
        for argument in guidee.arguments {
            commande.arg(argument);
        }
        commande.env("TERM", "xterm-256color");
        // Un dossier personnel introuvable n'empeche pas l'echange : le CLI partira du dossier
        // courant. On ne bloque donc pas dessus, mais on le journalise.
        match crate::chemins::dossier_personnel() {
            Ok(maison) => commande.cwd(maison),
            Err(e) => log::warn!("connexion guidee lancee sans dossier de depart : {e}"),
        }

        let enfant = paire
            .slave
            .spawn_command(commande)
            .map_err(|e| format!("lancement de {} : {e}", guidee.programme))?;
        drop(paire.slave);

        let tueur = enfant.clone_killer();
        let mut lecteur = paire.master.try_clone_reader().map_err(|e| e.to_string())?;
        let entree = paire.master.take_writer().map_err(|e| e.to_string())?;

        let vivant = Arc::new(std::sync::atomic::AtomicBool::new(true));
        {
            let vivant = vivant.clone();
            let app = app.clone();
            std::thread::spawn(move || {
                let mut morceau = [0u8; 4096];
                loop {
                    match lecteur.read(&mut morceau) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let texte = String::from_utf8_lossy(&morceau[..n]).to_string();
                            let _ = app.emit(EVENEMENT_SORTIE, texte);
                        }
                    }
                }
                vivant.store(false, std::sync::atomic::Ordering::SeqCst);
                let _ = app.emit(EVENEMENT_FIN, ());
            });
        }

        *self.session.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(EnCours { entree, tueur, vivant, _maitre: paire.master });
        Ok(())
    }

    /// Renvoie au CLI ce que la personne a recopie.
    pub fn entrer(&self, donnee: &str) -> Result<(), String> {
        let mut garde = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let session = garde.as_mut().ok_or("aucune connexion en cours")?;
        session
            .entree
            .write_all(format!("{}\r", donnee.trim()).as_bytes())
            .map_err(|e| e.to_string())
    }

    pub fn annuler(&self) {
        if let Some(mut session) = self.session.lock().unwrap_or_else(|e| e.into_inner()).take() {
            if session.vivant.load(std::sync::atomic::Ordering::SeqCst) {
                let _ = session.tueur.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    /// Un fournisseur qui ne gere pas d'abonnement rend un etat qui le DIT, au lieu de « non
    /// connecte » — ce qui inviterait a chercher une connexion qui n'existe pas.
    #[test]
    fn un_fournisseur_sans_abonnement_le_dit() {
        let codex = crate::llm::par_id("codex").unwrap();
        let etat = super::etat(codex);
        assert!(!etat.gere_abonnement);
        assert!(!etat.connexion_guidee);
        assert!(!etat.connecte);
        assert_eq!(etat.nom, "Codex");
    }

    /// Et Claude, lui, l'annonce — meme sur une machine ou son CLI n'est pas installe : c'est
    /// l'ecran des reglages qui distingue les deux.
    #[test]
    fn claude_annonce_gerer_un_abonnement() {
        let claude = crate::llm::par_id("claude").unwrap();
        let etat = super::etat(claude);
        assert!(etat.gere_abonnement);
        assert!(etat.connexion_guidee);
    }
}
