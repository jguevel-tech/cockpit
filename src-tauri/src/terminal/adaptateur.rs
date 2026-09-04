//! L'application vue du service de terminaux : l'implementation du trait `Terminaux`
//! par-dessus `service::Client`.
//!
//! Etape C du chantier (`docs/portabilite/plan-terminaux.md`). Ce fichier est la couture
//! entre trois choses qui ne se connaissent pas :
//! - le SERVICE tient l'etat vivant (shells, ecrans, agent qui tourne) et survit a
//!   l'application ;
//! - SQLite tient le nom d'onglet et le projet, parce qu'eux doivent survivre a un
//!   redemarrage de la machine ;
//! - TAURI porte la sortie jusqu'au webview.
//!
//! Deux consequences a garder en tete en lisant :
//! 1. **`renommer` ne traverse pas le socket.** Le nom vit en base ; le mettre aussi dans
//!    le service ferait deux verites pour une seule chaine.
//! 2. **Une seule connexion pour toute l'application.** Le service autorise un abonne par
//!    session, et cette connexion l'est pour toutes : c'est ce qui permet aux xterm du
//!    pool de continuer a recevoir leur sortie meme quand leur onglet n'est pas affiche.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::interface::{Creation, ResultatRecherche, Taille, TerminalInfo, Terminaux};
use super::service::client::{arreter_le_service_incompatible, Client};
use super::service::protocole::{
    ActionRecherche as ActionService, ErreurPoignee, Pousse, Taille as TailleService,
};
use super::service::{lancement, reconcilier, tuyau};
use crate::storage::Database;

/// Ce que l'implementation doit garder pour rebrancher la sortie apres une reconnexion.
#[derive(Clone)]
struct Contexte {
    app: AppHandle,
    db: Database,
}

/// La charge de l'evenement `terminal_output`. Forme figee : le frontend la lit telle
/// quelle (`{ id, data }`, data en base64).
#[derive(Serialize, Clone)]
struct SortiePayload {
    id: i64,
    data: String,
}

/// Temps minimum entre deux series de photos d'ecran, hors fermeture de la fenetre.
///
/// Une minute : assez pour qu'un aller-retour entre onglets ne coute rien, assez court pour
/// qu'une panne ne fasse pas perdre une heure de terminal.
const ENTRE_DEUX_PHOTOS: std::time::Duration = std::time::Duration::from_secs(60);

pub struct TerminauxService {
    /// Ou joindre le service. Une erreur ici (dossier d'execution inutilisable) est gardee
    /// telle quelle et remontee a la premiere operation, plutot que d'echouer au demarrage.
    chemin: Result<PathBuf, String>,
    client: Mutex<Option<Arc<Client>>>,
    /// Serialise les LANCEMENTS du service. Il existe pour que le verrou de `client` ne soit
    /// jamais tenu pendant un demarrage — voir `client()`.
    relance: Mutex<()>,
    /// La derniere panne CONSIGNEE, pour ne pas remplir le journal toutes les cinq secondes
    /// tant que la meme panne dure.
    derniere_panne: Mutex<Option<String>>,
    contexte: Mutex<Option<Contexte>>,
    /// Terminaux dont la sortie remonte deja. Sert a rendre `attacher` gratuit quand
    /// l'onglet revient : re-attacher declencherait un redessin complet, donc un
    /// clignotement et une perte de la position de defilement a chaque changement d'onglet.
    attaches: Mutex<HashSet<i64>>,
    /// Quand les ecrans ont ete photographies pour la derniere fois. Voir `assez_attendu`.
    derniere_photo: Mutex<Option<std::time::Instant>>,
    /// Conversations d'agent deja rendues a un terminal depuis le lancement. Sert a ne pas
    /// ouvrir deux fois la meme dans deux terminaux du meme projet.
    conversations_reprises: Mutex<HashSet<String>>,
}

impl Default for TerminauxService {
    fn default() -> Self {
        Self {
            chemin: chemin_socket(),
            client: Mutex::new(None),
            relance: Mutex::new(()),
            derniere_panne: Mutex::new(None),
            contexte: Mutex::new(None),
            attaches: Mutex::new(HashSet::new()),
            derniere_photo: Mutex::new(None),
            conversations_reprises: Mutex::new(HashSet::new()),
        }
    }
}

/// Le socket du service, avec un service PROPRE a une installation de developpement.
///
/// `COCKPIT_TERMINAUX_SOCKET` l'emporte toujours. Sinon, `COCKPIT_DB` (mode developpement
/// ou recette) donne son propre socket : sans cela, la reconciliation du demarrage verrait
/// les terminaux de l'installation NORMALE comme des sessions orphelines — la base de
/// developpement ne les connait pas — et les tuerait. C'est exactement le scenario qui a
/// coute des sessions vieilles de plusieurs jours du temps de tmux, ou il fallait une
/// exception en dur dans la purge.
fn chemin_socket() -> Result<PathBuf, String> {
    let defaut = tuyau::chemin().map_err(|e| e.to_string())?;
    if std::env::var_os(tuyau::VARIABLE_SOCKET).is_some() {
        return Ok(defaut);
    }
    let Some(base) = std::env::var_os("COCKPIT_DB") else {
        return Ok(defaut);
    };
    Ok(defaut.with_file_name(format!("terminaux-{}.sock", empreinte(&base.to_string_lossy()))))
}

/// Une empreinte courte et stable d'un chemin, pour nommer un socket sans y recopier des
/// caracteres qui n'y ont pas leur place.
fn empreinte(texte: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    texte.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn b64(donnees: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(donnees)
}

/// Ecrit dans le journal local. Une panne de fond n'a pas d'ecran ou s'afficher : elle
/// serait perdue sans ca, et c'est justement ce qu'on reproche a un `catch` muet.
fn journaliser(app: &AppHandle, scope: &str, message: &str) {
    if let Ok(dir) = app.path().app_data_dir() {
        let horodatage = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        crate::report::append_log(&dir, &crate::report::format_log_line(&horodatage, scope, message));
    }
}

/// Ce que le service envoie de lui-meme, traduit pour le frontend.
fn traiter_poussee(contexte: &Contexte, pousse: Pousse) {
    match pousse {
        // Sortie brute et redessin partent par le MEME evenement : le frontend les donne
        // tels quels a xterm, qui n'a pas a savoir lequel des deux il recoit.
        Pousse::Sortie { id, octets } | Pousse::Redessin { id, octets } => {
            let _ = contexte
                .app
                .emit("terminal_output", SortiePayload { id, data: b64(&octets) });
        }
        Pousse::PressePapier { id, texte } => {
            if let Err(e) = crate::poser_presse_papier(texte) {
                journaliser(&contexte.app, "terminal.pressePapier", &format!("terminal {id} : {e}"));
            }
        }
        Pousse::Fini { id } => {
            let _ = contexte.db.delete_terminal_row(id);
            let _ = contexte.app.emit("terminal_exit", id);
        }
        Pousse::Panne { id, message } => {
            journaliser(&contexte.app, "terminal.panne", &format!("terminal {id} : {message}"));
        }
    }
}

impl TerminauxService {
    fn chemin(&self) -> Result<&std::path::Path, String> {
        self.chemin.as_deref().map_err(|e| e.clone())
    }

    /// Consigne une panne du service, UNE SEULE FOIS tant qu'elle ne change pas.
    ///
    /// Sans borne, l'interrogation periodique de la liste des terminaux ecrirait la meme
    /// ligne toutes les cinq secondes et noierait le journal — celui-la meme qu'on lira au
    /// prochain incident.
    fn consigner_la_panne(&self, panne: &str) {
        let mut derniere = self.derniere_panne.lock().unwrap_or_else(|e| e.into_inner());
        if derniere.as_deref() == Some(panne) {
            return;
        }
        *derniere = Some(panne.to_string());
        if let Some(contexte) = self.contexte.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            journaliser(&contexte.app, "terminal.service", panne);
        }
    }

    fn panne_terminee(&self) {
        let mut derniere = self.derniere_panne.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(panne) = derniere.take() {
            if let Some(contexte) = self.contexte.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
                journaliser(
                    &contexte.app,
                    "terminal.service",
                    &format!("le service repond de nouveau (panne precedente : {panne})"),
                );
            }
        }
    }

    /// La connexion SI elle est deja etablie et vivante. Le verrou n'est tenu que le temps
    /// d'un clone : aucune entree-sortie derriere lui.
    fn deja_connecte(&self) -> Option<Arc<Client>> {
        let garde = self.client.lock().unwrap_or_else(|e| e.into_inner());
        match garde.as_ref() {
            Some(client) if client.vivant() => Some(Arc::clone(client)),
            _ => None,
        }
    }

    /// La connexion au service, lancee ou relancee si besoin.
    ///
    /// C'est ici que se tient la promesse « une application qui redemarre retrouve ses
    /// sessions » : le service tourne deja, on se rebranche dessus.
    ///
    /// **LE VERROU DE `client` N'EST JAMAIS TENU PENDANT UN LANCEMENT.** Lancer le service
    /// prend jusqu'a dix secondes, et la frappe prend ce meme verrou depuis la boucle
    /// graphique : le tenir gelait la fenetre entiere jusqu'au kill. `relance` garantit
    /// qu'un seul fil lance le service a la fois, sans rien bloquer d'autre.
    fn client(&self) -> Result<Arc<Client>, String> {
        if let Some(client) = self.deja_connecte() {
            return Ok(client);
        }
        let _un_seul_lancement = self.relance.lock().unwrap_or_else(|e| e.into_inner());
        // Un autre fil a pu reussir pendant qu'on attendait notre tour.
        if let Some(client) = self.deja_connecte() {
            return Ok(client);
        }
        let contexte = self
            .contexte
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or("le serveur de terminaux n'est pas encore pret")?;
        let chemin = self.chemin()?.to_path_buf();
        // Nomme l'attente : si la fenetre se fige pendant ce temps, le journal dira quoi.
        let _marque = crate::guetteur::marquer("lancement du service de terminaux");
        lancement::demarrer(&chemin)?;
        let contexte_pour_relance = contexte.clone();
        let client = match Client::connecter(&chemin, move |pousse| traiter_poussee(&contexte, pousse))
        {
            Ok(client) => client,
            // Un service d'une AUTRE version : il faut le remplacer, pas abandonner. Sans
            // cela l'application resterait bloquee jusqu'au prochain redemarrage de la
            // machine — les versions se comparent a l'egalite stricte, et un service qui
            // repond deja n'est jamais relance. Ses shells meurent avec lui : deux services
            // ne peuvent pas se passer des pseudo-terminaux vivants.
            Err(
                e @ (ErreurPoignee::ServiceTropAncien { .. }
                | ErreurPoignee::ApplicationTropAncienne { .. }),
            ) => {
                journaliser(
                    &contexte_pour_relance.app,
                    "terminal.service",
                    &format!("{e} — arret de l'ancien service et relance"),
                );
                arreter_le_service_incompatible(&chemin)?;
                lancement::demarrer(&chemin)?;
                let contexte = contexte_pour_relance.clone();
                Client::connecter(&chemin, move |pousse| traiter_poussee(&contexte, pousse))
                    .map_err(|e| e.to_string())?
            }
            Err(e) => return Err(e.to_string()),
        };
        // Un service neuf n'a plus aucun abonnement : tout onglet devra se rebrancher.
        self.attaches.lock().unwrap_or_else(|e| e.into_inner()).clear();
        *self.client.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::clone(&client));
        Ok(client)
    }

    /// La connexion SI elle est deja etablie. Le chemin de frappe passe par ici : relancer
    /// un service depuis une touche ferait attendre l'utilisateur des secondes pour un
    /// terminal qui, de toute facon, n'existe plus.
    fn client_vivant(&self) -> Result<Arc<Client>, String> {
        // `try_lock` et NON `lock` : ce chemin est celui de la frappe, portee par une
        // commande sans `async` — donc executee en ligne sur la boucle graphique. Attendre
        // ici gele la fenetre entiere. Un verrou occupe signifie qu'une connexion s'etablit :
        // on le dit tout de suite au lieu de faire attendre l'utilisateur devant une
        // interface morte.
        let garde = match self.client.try_lock() {
            Ok(garde) => garde,
            Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => {
                return Err("le service de terminaux est occupe a se connecter".into())
            }
        };
        match garde.as_ref() {
            Some(client) if client.vivant() => Ok(Arc::clone(client)),
            _ => Err("le service de terminaux ne repond plus".into()),
        }
    }
}

impl TerminauxService {
    /// Assez de temps depuis la derniere photo ? Retient l'instant si oui.
    ///
    /// Le geste qui declenche une photo (quitter la vue des terminaux) peut se repeter
    /// plusieurs fois par minute. Sans cette borne, chaque aller-retour entre deux onglets
    /// ferait relire l'ecran de tous les terminaux et reecrire la base.
    fn assez_attendu(&self) -> bool {
        let mut derniere = self.derniere_photo.lock().unwrap_or_else(|e| e.into_inner());
        let maintenant = std::time::Instant::now();
        if derniere.is_some_and(|quand| maintenant.duration_since(quand) < ENTRE_DEUX_PHOTOS) {
            return false;
        }
        *derniere = Some(maintenant);
        true
    }

    /// Rouvre un shell pour une ligne que le service ne connait plus, avec l'ecran d'avant.
    ///
    /// C'est le chemin normal apres une extinction du poste. Trois choix qui se lisent mal
    /// dans le code :
    /// - **le dossier vient de la LIGNE**, pas du projet : un terminal ouvert dans un
    ///   worktree ou un sous-dossier doit y revenir. La racine du projet n'est qu'un repli ;
    /// - **aucune commande initiale n'est rejouee.** Une commande relancee a l'aveugle peut
    ///   etre destructive, et personne ne l'a demandee au demarrage ;
    /// - **la photo n'est PAS effacee apres usage**, et c'est deliberе. L'effacer paraissait
    ///   propre ; en realite elle est de toute facon remplacee a la prochaine photo, et la
    ///   perdre ici ferait perdre l'ecran pour de bon si la machine s'arretait brutalement
    ///   entre la restauration et la photo suivante.
    ///
    /// Limite connue et acceptee : restaurer puis refermer sans rien taper empile un
    /// separateur et une invite de plus dans la photo suivante. L'historique etant borne en
    /// cellules, ca ne grossit pas sans fin et le contenu utile reste lisible.
    fn restaurer(&self, db: &Database, id: i64, taille: Taille) -> Result<(), String> {
        let client = self.client()?;
        let ligne = db.get_terminal_row(id)?;
        let dossier = if ligne.cwd.is_empty() {
            db.get_project_by_name(&ligne.project).map(|p| p.path).unwrap_or_default()
        } else {
            ligne.cwd
        };
        let reprise = self.reprise_de_l_agent(&ligne.agent, &dossier);
        client.creer(
            id,
            &dossier,
            taille_service(taille),
            reprise,
            db.get_terminal_snapshot(id),
        )?;
        Ok(())
    }

    /// La commande qui remet l'agent d'un terminal sur sa conversation, s'il y en avait un.
    ///
    /// **C'EST LA SEULE COMMANDE QUE LA RESTAURATION REJOUE, ET LA DISTINCTION EST LA
    /// SUIVANTE** : une commande de projet agit (elle construit, elle deploie, elle efface),
    /// donc elle ne se rejoue jamais toute seule ; la reprise d'un agent ouvre une
    /// conversation et ATTEND une consigne. Elle ne fait rien d'elle-meme.
    ///
    /// Trois raisons de ne rien rendre, toutes normales : aucun agent ne tournait, le
    /// fournisseur ne sait pas retrouver ses conversations passees (capacite `None` — on
    /// n'invente rien a sa place), ou ce dossier n'en a aucune. Dans ces cas le terminal
    /// s'ouvre en shell nu, et le bouton des conversations reste a un clic.
    ///
    /// **Une conversation n'est rendue qu'UNE FOIS par lancement de l'application.** Deux
    /// terminaux du meme projet reprennent donc les deux conversations les plus recentes,
    /// chacun la sienne, au lieu d'ouvrir deux fois la meme. Le service ne dit pas quelle
    /// conversation tournait dans quel terminal, et le deviner par les fichiers ouverts du
    /// processus serait vrai sous Linux seulement.
    fn reprise_de_l_agent(&self, agent: &str, dossier: &str) -> Option<String> {
        if agent.is_empty() || dossier.is_empty() {
            return None;
        }
        let fournisseur = crate::llm::par_id(agent)?;
        let lecteur = fournisseur.conversations()?;
        let passees = lecteur.lister(std::path::Path::new(dossier), crate::llm::conversations::MAX);
        let passees = match passees {
            Ok(passees) => passees,
            // Un dossier de conversations illisible ne doit pas empecher le terminal de
            // revenir : on le dit au journal et on ouvre un shell.
            Err(e) => {
                if let Some(contexte) = self.contexte.lock().unwrap_or_else(|e| e.into_inner()).as_ref()
                {
                    journaliser(&contexte.app, "terminal.repriseAgent", &e);
                }
                return None;
            }
        };
        let mut deja = self.conversations_reprises.lock().unwrap_or_else(|e| e.into_inner());
        let choisie = passees.into_iter().find(|c| !deja.contains(&c.id))?;
        deja.insert(choisie.id.clone());
        Some(lecteur.commande_de_reprise(&choisie.id))
    }
}

fn taille_service(taille: Taille) -> TailleService {
    TailleService { colonnes: taille.colonnes, lignes: taille.lignes }
}

impl Terminaux for TerminauxService {
    fn preparer(&self, app: &AppHandle, db: &Database) {
        *self.contexte.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(Contexte { app: app.clone(), db: db.clone() });

        let client = match self.client() {
            Ok(client) => client,
            // Sans service, aucun terminal ne s'ouvrira : l'erreur ressortira au premier
            // clic, avec un message. Ici, elle va au journal — il n'y a pas encore
            // d'interface pour l'afficher.
            Err(e) => return journaliser(app, "terminal.service", &e),
        };

        // Reconciliation : des qu'un etat survit a l'application, les deux divergent.
        let sessions = match client.lister() {
            Ok(sessions) => sessions,
            Err(e) => return journaliser(app, "terminal.reconciliation", &e),
        };
        let lignes: Vec<i64> = db
            .get_terminal_rows(None)
            .unwrap_or_default()
            .into_iter()
            .map(|row| row.id)
            .collect();
        let vue = reconcilier(&sessions, &lignes);
        // Les lignes sans session ne sont PAS supprimees : c'est l'etat normal apres une
        // extinction, et leur onglet doit reparaitre. Chacune retrouvera un shell a
        // l'ouverture de son onglet (voir `restaurer`) — jamais avant, sinon ouvrir Cockpit
        // lancerait un shell par terminal de chaque projet.
        let _ = &vue.lignes_a_restaurer;
        // Une session que plus aucun onglet ne peut afficher tourne pour personne.
        for id in vue.sessions_orphelines {
            if let Err(e) = client.fermer(id) {
                journaliser(app, "terminal.orpheline", &format!("terminal {id} : {e}"));
            }
        }
    }

    fn creer(&self, db: &Database, demande: Creation) -> Result<i64, String> {
        let client = self.client()?;
        let Creation { projet, dossier, taille, commande_initiale } = demande;
        // La ligne d'abord : c'est son rowid qui identifie la session, et lui seul traverse
        // l'extinction du poste. Le dossier est retenu avec elle — un terminal ouvert dans un
        // worktree ou un sous-dossier doit y revenir, pas a la racine du projet.
        let row = db.create_terminal_row_dans(&projet, &dossier)?;
        match client.creer(
            row.id,
            &dossier,
            taille_service(taille),
            commande_initiale,
            Vec::new(),
        ) {
            Ok(()) => Ok(row.id),
            Err(e) => {
                let _ = db.delete_terminal_row(row.id);
                Err(e)
            }
        }
    }

    fn ecrire(&self, id: i64, donnees: &str) -> Result<(), String> {
        self.client_vivant()?.ecrire(id, donnees.as_bytes())
    }

    fn redimensionner(&self, _db: &Database, id: i64, taille: Taille) -> Result<(), String> {
        self.client()?.redimensionner(id, taille_service(taille))
    }

    fn fermer(&self, db: &Database, id: i64) -> Result<(), String> {
        let client = self.client()?;
        if let Err(e) = client.fermer(id) {
            // Le service ne connait plus cette session : son shell est deja mort. La ligne
            // doit partir quand meme, sinon l'onglet ne peut plus se fermer. En revanche si
            // la session EST la et que la fermeture a echoue, on garde tout : supprimer la
            // ligne laisserait un shell vivant que plus personne ne peut joindre.
            let connue = client.lister().is_ok_and(|s| s.iter().any(|s| s.id == id));
            if connue {
                return Err(e);
            }
        }
        self.attaches.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
        db.delete_terminal_row(id)
    }

    fn attacher(&self, db: &Database, id: i64, taille: Taille) -> Result<(), String> {
        let client = self.client()?;
        // DEJA BRANCHE = ne rien faire. Le frontend appelle `attach` a chaque retour sur un
        // onglet ; refaire l'operation redemanderait un redessin complet, donc un
        // clignotement et un retour en bas de l'historique a chaque changement d'onglet.
        if self.attaches.lock().unwrap_or_else(|e| e.into_inner()).contains(&id) {
            return Ok(());
        }
        match client.attacher(id, taille_service(taille)) {
            Ok(()) => {
                self.attaches.lock().unwrap_or_else(|e| e.into_inner()).insert(id);
                Ok(())
            }
            Err(e) => {
                // Le service ne connait pas cette session. Le cas ORDINAIRE, apres une
                // extinction du poste : la ligne est la, le shell est mort avec la machine.
                // On rouvre un shell dans le meme dossier et on lui redonne l'ecran d'avant,
                // au lieu de supprimer l'onglet sous les yeux de l'utilisateur.
                if client.lister().is_ok_and(|s| s.iter().any(|s| s.id == id)) {
                    return Err(e);
                }
                self.restaurer(db, id, taille)?;
                client.attacher(id, taille_service(taille))?;
                self.attaches.lock().unwrap_or_else(|e| e.into_inner()).insert(id);
                Ok(())
            }
        }
    }

    /// Photographie tous les terminaux que le service tient, et range les photos en base.
    ///
    /// **AUCUN MINUTEUR ICI, ET C'EST DELIBERE.** Une photo coute un aller-retour par
    /// terminal et jusqu'a un mega-octet ecrit en base, et ce cout se paie MULTIPLIE par le
    /// nombre de terminaux ouverts — la meme regle que la surveillance des conteneurs, qui a
    /// deja fait tenir 200 % de processeur a docker. Les photos sont donc prises sur les
    /// GESTES : quand on quitte la vue des terminaux, et quand la fenetre se ferme. Entre
    /// deux, `assez_attendu` refuse de recommencer.
    ///
    /// Ne rend pas d'erreur : c'est un filet, pas une fonctionnalite. Un service deja parti
    /// signifie simplement qu'il n'y a plus rien a photographier.
    fn enregistrer_les_ecrans(&self, db: &Database, force: bool) {
        if !force && !self.assez_attendu() {
            return;
        }
        let Some(client) = self.deja_connecte() else { return };
        let Ok(sessions) = client.lister() else { return };
        // Le fournisseur est lu UNE fois : c'est un reglage, il ne change pas pendant la
        // boucle, et le relire par terminal coutait une requete de plus a chaque tour.
        let fournisseur = crate::llm::prefere(db).id();
        for session in sessions.iter().filter(|s| s.vivant) {
            match client.instantane(session.id) {
                Ok(octets) if !octets.is_empty() => {
                    // `llm` dit qu'un agent tourne, jamais LEQUEL : le service ne fait pas
                    // voyager cette information, et la faire voyager couterait une version de
                    // protocole, donc les terminaux detaches une fois. On retient donc le
                    // fournisseur PREFERE au moment de la photo. C'est juste dans le cas
                    // ordinaire — un seul reglage decide et tout le reste le lit — et faux
                    // seulement si l'on fait tourner a la main un fournisseur qui n'est pas
                    // celui qu'on a choisi.
                    let agent = if session.llm { fournisseur } else { "" };
                    let _ = db.set_terminal_snapshot(session.id, &octets, agent);
                }
                _ => {}
            }
        }
    }

    fn renommer(&self, db: &Database, id: i64, nom: &str) -> Result<(), String> {
        let propre: String = nom.trim().chars().take(40).collect();
        db.rename_terminal_row(id, &propre)
    }

    fn lister(&self, db: &Database, projet: Option<&str>) -> Vec<TerminalInfo> {
        // Un echec rendait ici une liste vide EN SILENCE, ce qui faisait passer TOUS les
        // terminaux pour morts sans qu'une ligne ne le dise nulle part. C'est le genre de
        // silence qui rend un incident indiagnosticable.
        let vivantes = match self.client().and_then(|c| c.lister()) {
            Ok(sessions) => {
                self.panne_terminee();
                sessions
            }
            Err(e) => {
                self.consigner_la_panne(&e);
                Vec::new()
            }
        };
        db.get_terminal_rows(projet)
            .unwrap_or_default()
            .into_iter()
            .map(|row| {
                let session = vivantes.iter().find(|s| s.id == row.id);
                TerminalInfo {
                    id: row.id,
                    project: row.project,
                    name: row.name,
                    alive: session.is_some_and(|s| s.vivant),
                    llm: session.is_some_and(|s| s.llm),
                }
            })
            .collect()
    }

    fn chercher(
        &self,
        _db: &Database,
        id: i64,
        action: super::interface::ActionRecherche,
        motif: &str,
    ) -> Result<ResultatRecherche, String> {
        use super::interface::ActionRecherche as A;
        let action = match action {
            A::Demarrer => ActionService::Demarrer,
            A::Suivante => ActionService::Suivante,
            A::Precedente => ActionService::Precedente,
            A::Annuler => ActionService::Annuler,
        };
        let (total, index, occurrence) = self.client()?.chercher(id, action, motif)?;
        Ok(ResultatRecherche {
            total,
            index,
            ligne: occurrence.map(|o| o.ligne),
            colonne: occurrence.map(|o| o.colonne),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LA QUESTION : un terminal qui faisait tourner un agent le retrouve-t-il SUR SA
    /// CONVERSATION, et deux terminaux du meme projet en ont-ils chacun une ?
    ///
    /// Tout est dans UN essai parce qu'il pose `HOME` : deux essais qui touchent
    /// l'environnement du processus se marchent dessus en parallele. Meme raison que pour le
    /// mode de rendu.
    ///
    /// Ce qui est verifie ici, et qui tombe si la reprise disparait : la commande rendue
    /// nomme la conversation la PLUS RECENTE, une conversation n'est rendue qu'une fois, et
    /// les trois cas ou l'on ne rejoue RIEN (pas d'agent, fournisseur inconnu, aucune
    /// conversation) ouvrent un shell nu.
    #[test]
    fn un_agent_est_repris_sur_sa_conversation_la_plus_recente() {
        let maison = std::env::temp_dir().join(format!("cockpit-reprise-{}", std::process::id()));
        let projet = maison.join("projet");
        let conversations = maison
            .join(".claude/projects")
            .join(projet.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect::<String>());
        std::fs::create_dir_all(&conversations).expect("dossier des conversations");
        std::fs::create_dir_all(&projet).expect("dossier du projet");

        // Deux conversations, la seconde plus recente. Le contenu importe peu : seul le nom
        // du fichier porte l'identifiant.
        let ancienne = conversations.join("11111111-1111-4111-8111-111111111111.jsonl");
        let recente = conversations.join("22222222-2222-4222-8222-222222222222.jsonl");
        std::fs::write(&ancienne, "{\"type\":\"user\",\"message\":{\"content\":\"vieille\"}}\n").unwrap();
        std::fs::write(&recente, "{\"type\":\"user\",\"message\":{\"content\":\"fraiche\"}}\n").unwrap();
        // Des dates franches : une egalite de seconde rendrait l'ordre indecidable.
        let vieux = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        filetime_grossier(&ancienne, vieux);

        // `HOME` sous Unix, `USERPROFILE` sous Windows : `dossier_personnel()` ne lit jamais
        // `HOME` la-bas.
        let ancien_home = std::env::var_os("HOME");
        let ancien_profil = std::env::var_os("USERPROFILE");
        std::env::set_var("HOME", &maison);
        std::env::set_var("USERPROFILE", &maison);

        let service = TerminauxService::default();
        let dossier = projet.to_string_lossy().to_string();

        // Un shell ordinaire ne rejoue RIEN : c'est la regle, et une commande de projet
        // relancee a l'aveugle peut etre destructive.
        assert_eq!(service.reprise_de_l_agent("", &dossier), None);
        // Un fournisseur retire du catalogue non plus.
        assert_eq!(service.reprise_de_l_agent("fournisseur-inconnu", &dossier), None);
        // Ni un dossier qu'on ne connait pas.
        assert_eq!(service.reprise_de_l_agent("claude", "/dossier/qui/n/existe/pas"), None);

        // Et l'agent est repris sur la conversation la plus recente.
        let premiere = service.reprise_de_l_agent("claude", &dossier);
        assert_eq!(
            premiere.as_deref(),
            Some("claude --resume 22222222-2222-4222-8222-222222222222"),
            "la conversation la plus recente doit etre reprise"
        );

        // Un SECOND terminal du meme projet prend la suivante, jamais la meme : sinon deux
        // onglets ouvriraient la meme conversation.
        let seconde = service.reprise_de_l_agent("claude", &dossier);
        assert_eq!(
            seconde.as_deref(),
            Some("claude --resume 11111111-1111-4111-8111-111111111111"),
            "le second terminal doit prendre l'autre conversation"
        );

        // Il n'y en a plus que deux : le troisieme terminal s'ouvre en shell nu.
        assert_eq!(service.reprise_de_l_agent("claude", &dossier), None);

        match ancien_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match ancien_profil {
            Some(v) => std::env::set_var("USERPROFILE", v),
            None => std::env::remove_var("USERPROFILE"),
        }
        let _ = std::fs::remove_dir_all(&maison);
    }

    /// Vieillit un fichier, sans dependance : on reecrit son contenu puis on repose sa date
    /// par le systeme. `filetime` n'est pas dans les dependances et n'a pas a y entrer pour
    /// un essai.
    fn filetime_grossier(chemin: &std::path::Path, quand: std::time::SystemTime) {
        let fichier = std::fs::File::options().write(true).open(chemin).expect("ouverture");
        fichier.set_modified(quand).expect("date de modification");
    }

    /// Deux bases differentes = deux services. C'est ce qui protege les terminaux de
    /// l'installation normale quand une installation de developpement demarre a cote.
    #[test]
    fn une_base_choisie_a_la_main_a_son_propre_socket() {
        let a = empreinte("/home/moi/.local/share/com.cockpit.dev/data.db");
        let b = empreinte("/tmp/recette/data.db");
        assert_ne!(a, b);
        // Stable d'un appel a l'autre : le service se retrouve au redemarrage de l'app.
        assert_eq!(a, empreinte("/home/moi/.local/share/com.cockpit.dev/data.db"));
        assert_eq!(a.len(), 16);
    }

    /// Le nom reste un nom de fichier : pas de separateur venu du chemin de la base.
    #[test]
    fn l_empreinte_ne_contient_que_des_chiffres_hexadecimaux() {
        assert!(empreinte("/un/chemin/quelconque").chars().all(|c| c.is_ascii_hexdigit()));
    }
}
