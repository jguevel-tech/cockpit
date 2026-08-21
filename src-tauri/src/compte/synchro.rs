//! Le va-et-vient avec le serveur : on envoie ce qui a change ici, on applique ce qui a
//! change ailleurs.
//!
//! **Un seul appel fait les deux.** Deux appels separes laisseraient une fenetre entre l'envoi
//! et la lecture, pendant laquelle une modification recue pourrait etre ecrasee par un envoi
//! parti avant elle.
//!
//! Rien ici n'est sur le chemin d'une fonctionnalite : une panne se journalise et se raconte
//! dans les reglages, elle n'interrompt rien.

use super::{jeton, motif, serveur};
use crate::storage::db::Database;
use crate::storage::synchro::ChangementLocal;
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Reglage ou vit le curseur : le numero du dernier changement recu du serveur.
const CLE_CURSEUR: &str = "compte_synchro_curseur";
/// Reglage ou vit l'instant du dernier passage reussi.
const CLE_DERNIER_PASSAGE: &str = "compte_synchro_dernier";

/// Plafond d'un envoi. Le serveur en impose un ; le depasser ferait refuser tout le lot.
const ENVOI_MAXIMUM: usize = 500;

/// Nombre de tours au plus par passage.
///
/// Sans plafond, une premiere synchronisation d'un gros compte tournerait sans rendre la main.
/// Le passage suivant reprend la suite : rien n'est perdu, c'est seulement etale.
const TOURS_MAXIMUM: usize = 20;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Resultat {
    pub envoyes: usize,
    pub recus: usize,
    /// Faux quand il reste des choses a recuperer.
    pub complet: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EtatSynchro {
    pub actif: bool,
    pub en_attente: usize,
    pub dernier_passage: Option<i64>,
}

#[derive(Serialize)]
struct ModificationSortante {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    donnees: serde_json::Value,
    modifie_le: i64,
    supprime_le: Option<i64>,
}

#[derive(Deserialize)]
struct ModificationEntrante {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    donnees: serde_json::Value,
    supprime_le: Option<i64>,
}

#[derive(Deserialize)]
struct ReponseSynchro {
    jusqu_a: i64,
    complet: bool,
    modifications: Vec<ModificationEntrante>,
    /// Ce que le serveur a refuse parce qu'il detenait mieux. Il faut l'appliquer ICI, sinon
    /// la machine garde sa version perimee en croyant avoir gagne.
    refuses: Vec<ModificationEntrante>,
}

fn curseur(db: &Database) -> i64 {
    db.get_setting(CLE_CURSEUR)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// Remet la journalisation en marche quoi qu'il arrive.
///
/// La pause vit en base : sortir sans la lever — panne, erreur, retour anticipe — arreterait
/// DEFINITIVEMENT les envois de cette machine, sans que rien ne le signale.
struct Pause<'a>(&'a Database);

impl Drop for Pause<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.0.suspendre_la_journalisation(false) {
            log::error!("synchro : impossible de relancer la journalisation — {e}");
        }
    }
}

/// Prepare ce qui part.
fn a_envoyer(db: &Database, attente: &[ChangementLocal]) -> Vec<ModificationSortante> {
    let mut sortantes = Vec::new();

    for c in attente.iter().take(ENVOI_MAXIMUM) {
        let donnees = if c.supprime_le.is_some() {
            // Une suppression n'a pas de contenu : la ligne n'existe plus.
            serde_json::json!({})
        } else {
            match db.contenu_a_envoyer(&c.type_, &c.uuid) {
                Ok(Some(brut)) => serde_json::from_str(&brut).unwrap_or_else(|e| {
                    log::warn!("synchro : contenu illisible pour {} {} — {e}", c.type_, c.uuid);
                    serde_json::json!({})
                }),
                // La ligne a disparu entre le journal et l'envoi sans laisser de trace de
                // suppression : il n'y a rien a dire au serveur.
                Ok(None) => continue,
                Err(e) => {
                    log::warn!("synchro : lecture impossible pour {} {} — {e}", c.type_, c.uuid);
                    continue;
                }
            }
        };

        sortantes.push(ModificationSortante {
            id: c.uuid.clone(),
            type_: c.type_.clone(),
            donnees,
            modifie_le: c.modifie_le,
            supprime_le: c.supprime_le,
        });
    }

    sortantes
}

/// Ecrit ce qui vient du serveur, journalisation suspendue.
fn appliquer(db: &Database, recues: &[ModificationEntrante]) -> Result<usize, String> {
    if recues.is_empty() {
        return Ok(0);
    }

    db.suspendre_la_journalisation(true).map_err(|e| e.to_string())?;
    let _pause = Pause(db);

    let mut recousables = Vec::new();
    let mut appliquees = 0;

    for m in recues {
        let resultat = if m.supprime_le.is_some() {
            db.appliquer_une_suppression(&m.type_, &m.id)
        } else {
            db.appliquer_un_changement(&m.type_, &m.id, &m.donnees)
        };

        match resultat {
            Ok(()) => {
                appliquees += 1;
                if m.supprime_le.is_none() {
                    recousables.push((m.type_.clone(), m.id.clone(), m.donnees.clone()));
                }
            }
            // Une donnee qui ne rentre pas — nom deja pris, contrainte — ne doit pas arreter
            // les autres. Elle reviendra au passage suivant.
            Err(e) => log::warn!("synchro : « {} » {} refusee ici — {e}", m.type_, m.id),
        }
    }

    // Sans cette passe, un dossier recu avant son parent resterait a la racine pour toujours.
    if let Err(e) = db.recoudre_les_references(&recousables) {
        log::warn!("synchro : references non recousues — {e}");
    }

    Ok(appliquees)
}

async fn un_tour(
    db: &Database,
    client: &reqwest::Client,
    adresse: &str,
    jeton: &str,
) -> Result<(Resultat, Vec<ChangementLocal>), String> {
    let attente = db.changements_en_attente().map_err(|e| {
        log::warn!("synchro : journal illisible — {e}");
        motif::SERVEUR.to_string()
    })?;
    let sortantes = a_envoyer(db, &attente);

    let reponse = client
        .post(format!("{adresse}/api/synchro"))
        .bearer_auth(jeton)
        .json(&serde_json::json!({
            "depuis": curseur(db),
            "modifications": sortantes,
        }))
        .send()
        .await
        .map_err(|e| {
            log::warn!("synchro : serveur injoignable — {e}");
            motif::RESEAU.to_string()
        })?;

    if !reponse.status().is_success() {
        let code = reponse.status();
        log::warn!("synchro : le serveur a repondu {code}");
        return Err(if code.as_u16() == 401 {
            motif::IDENTIFIANTS.to_string()
        } else {
            motif::SERVEUR.to_string()
        });
    }

    let recue: ReponseSynchro = reponse.json().await.map_err(|e| {
        log::warn!("synchro : reponse illisible — {e}");
        motif::SERVEUR.to_string()
    })?;

    let recus = appliquer(db, &recue.modifications)? + appliquer(db, &recue.refuses)?;

    // Le curseur n'avance QU'APRES l'application : s'arreter entre les deux ferait sauter
    // definitivement ce qui vient d'arriver.
    db.set_setting(CLE_CURSEUR, &recue.jusqu_a.to_string())?;

    let envoyes = sortantes.len();
    let acquittes: Vec<ChangementLocal> = attente.into_iter().take(envoyes).collect();

    Ok((Resultat { envoyes, recus, complet: recue.complet }, acquittes))
}

/// Un passage complet : on tourne tant que le serveur dit qu'il en reste.
pub async fn passer(db: &Database) -> Result<Resultat, String> {
    let Some(jeton) = jeton(db) else {
        return Ok(Resultat { envoyes: 0, recus: 0, complet: true });
    };
    let adresse = serveur(db);
    let client = super::client();

    let mut total = Resultat { envoyes: 0, recus: 0, complet: true };

    for _ in 0..TOURS_MAXIMUM {
        let (tour, acquittes) = un_tour(db, client, &adresse, &jeton).await?;

        db.oublier_les_changements_envoyes(&acquittes)
            .map_err(|e| e.to_string())?;

        total.envoyes += tour.envoyes;
        total.recus += tour.recus;
        total.complet = tour.complet;

        // Plus rien a envoyer et plus rien a recevoir : c'est fini.
        if tour.complet && tour.envoyes == 0 {
            break;
        }
    }

    let maintenant = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    db.set_setting(CLE_DERNIER_PASSAGE, &maintenant.to_string())?;

    Ok(total)
}

#[tauri::command]
pub async fn synchro_maintenant(
    fenetre: tauri::Window,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Resultat, String> {
    let resultat = passer(&state.db).await?;

    // L'interface se recharge SEULEMENT si quelque chose est arrive : rafraichir a vide ferait
    // sauter la selection et le defilement pour rien.
    if resultat.recus > 0 {
        if let Err(e) = fenetre.emit("synchro-recue", resultat.recus) {
            log::warn!("synchro : l'interface n'a pas ete prevenue — {e}");
        }
    }

    Ok(resultat)
}

#[tauri::command]
pub async fn synchro_etat(state: tauri::State<'_, crate::AppState>) -> Result<EtatSynchro, String> {
    let db = &state.db;
    Ok(EtatSynchro {
        actif: jeton(db).is_some(),
        en_attente: db.changements_en_attente().map(|c| c.len()).unwrap_or(0),
        dernier_passage: db.get_setting(CLE_DERNIER_PASSAGE).and_then(|v| v.parse().ok()),
    })
}
