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
use super::service::client::Client;
use super::service::protocole::{ActionRecherche as ActionService, Pousse, Taille as TailleService};
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

pub struct TerminauxService {
    /// Ou joindre le service. Une erreur ici (dossier d'execution inutilisable) est gardee
    /// telle quelle et remontee a la premiere operation, plutot que d'echouer au demarrage.
    chemin: Result<PathBuf, String>,
    client: Mutex<Option<Arc<Client>>>,
    contexte: Mutex<Option<Contexte>>,
    /// Terminaux dont la sortie remonte deja. Sert a rendre `attacher` gratuit quand
    /// l'onglet revient : re-attacher declencherait un redessin complet, donc un
    /// clignotement et une perte de la position de defilement a chaque changement d'onglet.
    attaches: Mutex<HashSet<i64>>,
}

impl Default for TerminauxService {
    fn default() -> Self {
        Self {
            chemin: chemin_socket(),
            client: Mutex::new(None),
            contexte: Mutex::new(None),
            attaches: Mutex::new(HashSet::new()),
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

    /// La connexion au service, lancee ou relancee si besoin.
    ///
    /// C'est ici que se tient la promesse « une application qui redemarre retrouve ses
    /// sessions » : le service tourne deja, on se rebranche dessus.
    fn client(&self) -> Result<Arc<Client>, String> {
        let mut garde = self.client.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(client) = garde.as_ref() {
            if client.vivant() {
                return Ok(Arc::clone(client));
            }
        }
        let contexte = self
            .contexte
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .ok_or("le serveur de terminaux n'est pas encore pret")?;
        let chemin = self.chemin()?.to_path_buf();
        lancement::demarrer(&chemin)?;
        let client = Client::connecter(&chemin, move |pousse| traiter_poussee(&contexte, pousse))
            .map_err(|e| e.to_string())?;
        // Un service neuf n'a plus aucun abonnement : tout onglet devra se rebrancher.
        self.attaches.lock().unwrap_or_else(|e| e.into_inner()).clear();
        *garde = Some(Arc::clone(&client));
        Ok(client)
    }

    /// La connexion SI elle est deja etablie. Le chemin de frappe passe par ici : relancer
    /// un service depuis une touche ferait attendre l'utilisateur des secondes pour un
    /// terminal qui, de toute facon, n'existe plus.
    fn client_vivant(&self) -> Result<Arc<Client>, String> {
        let garde = self.client.lock().unwrap_or_else(|e| e.into_inner());
        match garde.as_ref() {
            Some(client) if client.vivant() => Ok(Arc::clone(client)),
            _ => Err("le service de terminaux ne repond plus".into()),
        }
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
        for id in vue.lignes_a_supprimer {
            let _ = db.delete_terminal_row(id);
        }
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
        // un redemarrage de la machine.
        let row = db.create_terminal_row(&projet)?;
        match client.creer(row.id, &dossier, taille_service(taille), commande_initiale) {
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
                // Le service ne connait pas cette session : sa ligne ne designe plus rien.
                if !client.lister().is_ok_and(|s| s.iter().any(|s| s.id == id)) {
                    let _ = db.delete_terminal_row(id);
                }
                Err(e)
            }
        }
    }

    fn renommer(&self, db: &Database, id: i64, nom: &str) -> Result<(), String> {
        let propre: String = nom.trim().chars().take(40).collect();
        db.rename_terminal_row(id, &propre)
    }

    fn lister(&self, db: &Database, projet: Option<&str>) -> Vec<TerminalInfo> {
        let vivantes = self.client().and_then(|c| c.lister()).unwrap_or_default();
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
