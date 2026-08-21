//! Compte utilisateur et dialogue avec le serveur de synchronisation.
//!
//! Tout est OPTIONNEL ici : sans compte, Cockpit fonctionne exactement pareil. Aucune fonction
//! de ce module ne doit devenir un passage oblige, et une panne reseau ne doit jamais empecher
//! quoi que ce soit d'autre de marcher.
//!
//! **Le jeton est range dans la table des reglages**, comme la cle OpenAI avant lui. C'est un
//! fichier du profil de l'utilisateur, en clair : quelqu'un qui lit ce fichier a deja acces a
//! tout le reste de ses donnees. Un trousseau systeme ferait mieux, mais il exige un service
//! de secrets qui n'existe pas partout, et son absence casserait la connexion la ou tout
//! marchait. A rouvrir le jour ou on accepte cette dependance.
//!
//! **Toutes les commandes de ce module sont `async`** : une commande synchrone s'execute dans
//! la boucle GTK et gele l'interface entiere — ici, le temps d'un aller-retour reseau.

use crate::storage::db::Database;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

/// Serveur interroge par defaut.
///
/// Modifiable dans les reglages : quelqu'un qui heberge le sien n'a pas a recompiler. La valeur
/// n'est pas un secret — c'est l'adresse publique du site.
pub const SERVEUR_PAR_DEFAUT: &str = "https://cockpit.86.253.219.203.sslip.io";

const CLE_SERVEUR: &str = "compte_serveur";
const CLE_JETON: &str = "compte_jeton";
const CLE_EMAIL: &str = "compte_email";
const CLE_NOM: &str = "compte_nom";
const CLE_APPAREIL: &str = "compte_appareil";

/// Au-dela, on rend la main : mieux vaut dire « le serveur ne repond pas » que laisser un
/// bouton tourner sans fin.
const DELAI: Duration = Duration::from_secs(15);

/// Motifs rendus a l'interface. Ce sont des CLES, pas des phrases : c'est l'interface qui sait
/// dans quelle langue et a quel endroit les afficher.
pub mod motif {
    pub const RESEAU: &str = "reseau";
    pub const SERVEUR: &str = "serveur";
    pub const IDENTIFIANTS: &str = "identifiants_invalides";
    pub const TROP_DE_TENTATIVES: &str = "trop_de_tentatives";
    pub const APPAIRAGE_EXPIRE: &str = "appairage_expire";
}

#[derive(Debug, Clone, Serialize)]
pub struct EtatCompte {
    pub connecte: bool,
    pub email: Option<String>,
    pub nom: Option<String>,
    pub serveur: String,
    /// Nom que cette machine annonce au serveur. Affiche pour qu'on reconnaisse ses machines
    /// dans la liste du compte.
    pub appareil: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DemandeAppairage {
    pub id: String,
    pub code: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "etat", rename_all = "snake_case")]
pub enum EtatAppairage {
    EnAttente,
    Accorde { compte: EtatCompte },
}

#[derive(Deserialize)]
struct ReponseCompte {
    jeton: String,
    compte: ProfilDistant,
}

#[derive(Deserialize)]
struct ProfilDistant {
    email: String,
    nom: Option<String>,
}

#[derive(Deserialize)]
struct ReponseAppairage {
    id: String,
    code: String,
    url: String,
}

#[derive(Deserialize)]
struct RefusServeur {
    erreur: Option<String>,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(DELAI)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

pub fn serveur(db: &Database) -> String {
    db.get_setting(CLE_SERVEUR)
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| SERVEUR_PAR_DEFAUT.to_string())
}

pub fn jeton(db: &Database) -> Option<String> {
    db.get_setting(CLE_JETON).filter(|v| !v.trim().is_empty())
}

/// La cle de cette machine, creee au premier besoin et jamais changee ensuite.
///
/// C'est ce qui permet au serveur de reconnaitre le poste apres un redemarrage, et donc de ne
/// pas lui renvoyer ce qu'il vient d'envoyer. En fabriquer une nouvelle a chaque connexion
/// remplirait la liste des machines du compte de doublons.
pub fn cle_appareil(db: &Database) -> Result<String, String> {
    if let Some(cle) = db.get_setting(CLE_APPAREIL).filter(|v| !v.trim().is_empty()) {
        return Ok(cle);
    }
    let cle = uuid::Uuid::new_v4().to_string();
    db.set_setting(CLE_APPAREIL, &cle)?;
    Ok(cle)
}

/// Le nom de la machine tel qu'il apparaitra dans la liste du compte.
pub fn nom_machine() -> String {
    sysinfo::System::host_name()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "machine".to_string())
}

pub fn systeme() -> &'static str {
    std::env::consts::OS
}

fn etat(db: &Database) -> EtatCompte {
    EtatCompte {
        connecte: jeton(db).is_some(),
        email: db.get_setting(CLE_EMAIL),
        nom: db.get_setting(CLE_NOM).filter(|v| !v.trim().is_empty()),
        serveur: serveur(db),
        appareil: nom_machine(),
    }
}

fn bloc_appareil(db: &Database) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "cle": cle_appareil(db)?,
        "nom": nom_machine(),
        "systeme": systeme(),
    }))
}

/// Traduit une reponse en echec en motif affichable.
///
/// Le corps est LU avant d'etre jete : le serveur nomme ses refus (« adresse_deja_prise »,
/// « mot_de_passe_trop_court »), et les remplacer par un message general obligerait
/// l'utilisateur a deviner ce qu'il doit corriger.
async fn refus(reponse: reqwest::Response) -> String {
    let code = reponse.status();
    let corps = reponse.text().await.unwrap_or_default();
    log::warn!("compte : le serveur a repondu {code} — {corps}");

    if let Ok(RefusServeur { erreur: Some(motif) }) = serde_json::from_str::<RefusServeur>(&corps) {
        return motif;
    }
    match code.as_u16() {
        401 => motif::IDENTIFIANTS.to_string(),
        403 | 429 => motif::TROP_DE_TENTATIVES.to_string(),
        _ => motif::SERVEUR.to_string(),
    }
}

fn panne_reseau(e: reqwest::Error) -> String {
    log::warn!("compte : le serveur est injoignable — {e}");
    motif::RESEAU.to_string()
}

async fn ouvrir_une_session(
    db: &Database,
    chemin: &str,
    email: &str,
    mot_de_passe: &str,
) -> Result<EtatCompte, String> {
    let corps = serde_json::json!({
        "email": email,
        "mot_de_passe": mot_de_passe,
        "appareil": bloc_appareil(db)?,
    });

    let reponse = client()
        .post(format!("{}{chemin}", serveur(db)))
        .json(&corps)
        .send()
        .await
        .map_err(panne_reseau)?;

    if !reponse.status().is_success() {
        return Err(refus(reponse).await);
    }

    let recue: ReponseCompte = reponse.json().await.map_err(|e| {
        log::warn!("compte : reponse illisible — {e}");
        motif::SERVEUR.to_string()
    })?;

    enregistrer_la_session(db, &recue)
}

fn enregistrer_la_session(db: &Database, recue: &ReponseCompte) -> Result<EtatCompte, String> {
    db.set_setting(CLE_JETON, &recue.jeton)?;
    db.set_setting(CLE_EMAIL, &recue.compte.email)?;
    db.set_setting(CLE_NOM, recue.compte.nom.as_deref().unwrap_or(""))?;
    Ok(etat(db))
}

#[tauri::command]
pub async fn compte_etat(state: tauri::State<'_, crate::AppState>) -> Result<EtatCompte, String> {
    Ok(etat(&state.db))
}

#[tauri::command]
pub async fn compte_inscription(
    email: String,
    mot_de_passe: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    ouvrir_une_session(&state.db, "/api/inscription", &email, &mot_de_passe).await
}

#[tauri::command]
pub async fn compte_connexion(
    email: String,
    mot_de_passe: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    ouvrir_une_session(&state.db, "/api/connexion", &email, &mot_de_passe).await
}

/// Demande un appairage : le logiciel ouvrira l'adresse rendue dans le navigateur.
///
/// Ce detour existe parce que Cockpit n'a pas de serveur HTTP — Google ne peut donc pas lui
/// renvoyer l'utilisateur sur une adresse locale.
#[tauri::command]
pub async fn compte_appairage_demarrer(
    state: tauri::State<'_, crate::AppState>,
) -> Result<DemandeAppairage, String> {
    let corps = serde_json::json!({ "appareil": bloc_appareil(&state.db)? });

    let reponse = client()
        .post(format!("{}/api/appairage", serveur(&state.db)))
        .json(&corps)
        .send()
        .await
        .map_err(panne_reseau)?;

    if !reponse.status().is_success() {
        return Err(refus(reponse).await);
    }

    let recue: ReponseAppairage = reponse.json().await.map_err(|e| {
        log::warn!("compte : demande d'appairage illisible — {e}");
        motif::SERVEUR.to_string()
    })?;

    Ok(DemandeAppairage { id: recue.id, code: recue.code, url: recue.url })
}

/// Le logiciel interroge cette commande jusqu'a obtenir son jeton.
#[tauri::command]
pub async fn compte_appairage_etat(
    id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatAppairage, String> {
    let reponse = client()
        .get(format!("{}/api/appairage/{id}", serveur(&state.db)))
        .send()
        .await
        .map_err(panne_reseau)?;

    match reponse.status().as_u16() {
        202 => Ok(EtatAppairage::EnAttente),
        410 => Err(motif::APPAIRAGE_EXPIRE.to_string()),
        code if (200..300).contains(&code) => {
            let recue: ReponseCompte = reponse.json().await.map_err(|e| {
                log::warn!("compte : reponse d'appairage illisible — {e}");
                motif::SERVEUR.to_string()
            })?;
            Ok(EtatAppairage::Accorde { compte: enregistrer_la_session(&state.db, &recue)? })
        }
        _ => Err(refus(reponse).await),
    }
}

/// Se deconnecter previent le serveur AVANT d'oublier le jeton.
///
/// Oublier le jeton sans le revoquer laisserait un acces valide dans la nature — c'est
/// precisement ce qu'une deconnexion promet d'empecher. Si le serveur est injoignable, on
/// oublie quand meme localement : refuser de se deconnecter hors ligne serait absurde. La
/// panne part alors dans les journaux.
#[tauri::command]
pub async fn compte_deconnexion(
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    let db = &state.db;

    if let Some(jeton) = jeton(db) {
        match client()
            .post(format!("{}/api/deconnexion", serveur(db)))
            .bearer_auth(jeton)
            .send()
            .await
        {
            Ok(reponse) if !reponse.status().is_success() => {
                log::warn!("compte : le serveur a refuse la deconnexion ({})", reponse.status());
            }
            Err(e) => log::warn!("compte : deconnexion hors ligne, le jeton reste a revoquer — {e}"),
            _ => {}
        }
    }

    db.set_setting(CLE_JETON, "")?;
    db.set_setting(CLE_EMAIL, "")?;
    db.set_setting(CLE_NOM, "")?;
    Ok(etat(db))
}

#[tauri::command]
pub async fn compte_definir_serveur(
    url: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    let propre = url.trim().trim_end_matches('/');

    // Un serveur en clair transporterait le mot de passe en clair. On refuse, sauf en local
    // ou il n'y a pas de reseau a ecouter — c'est le cas d'un developpement.
    let local = propre.starts_with("http://127.0.0.1") || propre.starts_with("http://localhost");
    if !propre.is_empty() && !propre.starts_with("https://") && !local {
        return Err("serveur_non_chiffre".to_string());
    }

    state.db.set_setting(CLE_SERVEUR, propre)?;
    Ok(etat(&state.db))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_de_la_machine_ne_change_pas() {
        let db = Database::new(":memory:").unwrap();

        let premiere = cle_appareil(&db).unwrap();
        let seconde = cle_appareil(&db).unwrap();

        assert_eq!(premiere, seconde, "une cle differente creerait un doublon a chaque connexion");
        assert_eq!(premiere.len(), 36, "un UUID canonique fait 36 caracteres");
    }

    #[test]
    fn le_serveur_par_defaut_sert_tant_que_rien_n_est_choisi() {
        let db = Database::new(":memory:").unwrap();
        assert_eq!(serveur(&db), SERVEUR_PAR_DEFAUT);

        db.set_setting("compte_serveur", "https://exemple.test").unwrap();
        assert_eq!(serveur(&db), "https://exemple.test");

        // Une valeur vide veut dire « remettre le defaut », pas « aucun serveur » : sinon
        // effacer le champ dans les reglages rendrait le compte inutilisable sans le dire.
        db.set_setting("compte_serveur", "  ").unwrap();
        assert_eq!(serveur(&db), SERVEUR_PAR_DEFAUT);
    }

    #[test]
    fn un_jeton_vide_vaut_pas_de_jeton() {
        let db = Database::new(":memory:").unwrap();
        assert!(jeton(&db).is_none());

        db.set_setting("compte_jeton", "ckpt_quelque_chose").unwrap();
        assert_eq!(jeton(&db).as_deref(), Some("ckpt_quelque_chose"));

        // C'est ce que la deconnexion ecrit : sans ce filtre, le logiciel se croirait encore
        // connecte et enverrait un en-tete vide a chaque appel.
        db.set_setting("compte_jeton", "").unwrap();
        assert!(jeton(&db).is_none());
    }

    #[test]
    fn le_nom_de_la_machine_n_est_jamais_vide() {
        assert!(!nom_machine().trim().is_empty());
    }
}
