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

pub mod synchro;

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
const CLE_INITIALES: &str = "compte_initiales";
/// L'avatar est garde EN LOCAL, sous forme d'adresse `data:`.
///
/// Pointer l'image du serveur marcherait tant qu'on est connecte, et afficherait un cadre vide
/// des qu'on ne l'est plus — sur un logiciel dont la promesse est de fonctionner hors ligne.
const CLE_AVATAR: &str = "compte_avatar";
/// L'adresse de l'image deja rangee. Elle porte un numero de version, donc la comparer suffit
/// a savoir si l'image a change ailleurs — sans la telecharger pour rien a chaque passage.
const CLE_AVATAR_ADRESSE: &str = "compte_avatar_adresse";
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
    pub const PAS_CONNECTE: &str = "pas_connecte";
}

#[derive(Debug, Clone, Serialize)]
pub struct EtatCompte {
    pub connecte: bool,
    pub email: Option<String>,
    pub nom: Option<String>,
    /// Une ou deux lettres, toujours presentes : sans image, il faut afficher quelque chose
    /// plutot qu'un rond vide, qui se lirait comme un defaut.
    pub initiales: Option<String>,
    /// Adresse `data:` de l'image, ou `None`.
    pub avatar: Option<String>,
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
    initiales: Option<String>,
    /// Adresse de l'image sur le serveur. On la telecharge pour la garder ici.
    avatar: Option<String>,
}

#[derive(Deserialize)]
struct ReponseProfil {
    compte: ProfilDistant,
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

pub(crate) fn client() -> &'static reqwest::Client {
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
    let vide = |c: &str| db.get_setting(c).filter(|v| !v.trim().is_empty());

    EtatCompte {
        connecte: jeton(db).is_some(),
        email: db.get_setting(CLE_EMAIL),
        nom: vide(CLE_NOM),
        initiales: vide(CLE_INITIALES),
        avatar: vide(CLE_AVATAR),
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
    nom: Option<&str>,
) -> Result<EtatCompte, String> {
    let mut corps = serde_json::json!({
        "email": email,
        "mot_de_passe": mot_de_passe,
        "appareil": bloc_appareil(db)?,
    });
    // Le nom n'accompagne que l'inscription : l'envoyer a la connexion ecraserait celui que
    // l'utilisateur a pu changer depuis, avec ce que ce poste-ci se rappelle.
    if let Some(nom) = nom.filter(|n| !n.trim().is_empty()) {
        corps["nom"] = serde_json::Value::String(nom.trim().to_string());
    }

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

    enregistrer_la_session(db, &recue).await
}

async fn enregistrer_la_session(db: &Database, recue: &ReponseCompte) -> Result<EtatCompte, String> {
    db.set_setting(CLE_JETON, &recue.jeton)?;

    rapatrier_l_avatar(db, recue.compte.avatar.as_deref()).await;
    enregistrer_le_profil(db, &recue.compte)
}

/// Range ce que le serveur dit du compte, image comprise.
fn enregistrer_le_profil(db: &Database, profil: &ProfilDistant) -> Result<EtatCompte, String> {
    db.set_setting(CLE_EMAIL, &profil.email)?;
    db.set_setting(CLE_NOM, profil.nom.as_deref().unwrap_or(""))?;
    db.set_setting(CLE_INITIALES, profil.initiales.as_deref().unwrap_or(""))?;
    Ok(etat(db))
}

/// Telecharge l'avatar et le garde en local.
///
/// Separe du reste parce qu'il peut echouer sans que ce soit grave : sans image on retombe sur
/// les initiales. Une panne ici ne doit pas faire echouer une connexion.
async fn rapatrier_l_avatar(db: &Database, adresse: Option<&str>) {
    let Some(adresse) = adresse else {
        let _ = db.set_setting(CLE_AVATAR, "");
        let _ = db.set_setting(CLE_AVATAR_ADRESSE, "");
        return;
    };

    // Rien a faire si c'est la MEME adresse et qu'on a deja l'image : le numero de version
    // qu'elle porte change des que l'image change. Sans cette comparaison, un passage de
    // synchronisation retelechargerait 85 Ko toutes les trois minutes pour rien.
    let deja = db.get_setting(CLE_AVATAR_ADRESSE).unwrap_or_default();
    let garde = db.get_setting(CLE_AVATAR).unwrap_or_default();
    if deja == adresse && !garde.is_empty() {
        return;
    }

    match client().get(adresse).send().await {
        Ok(reponse) if reponse.status().is_success() => {
            let type_ = reponse
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("image/png")
                .to_string();
            match reponse.bytes().await {
                Ok(octets) => {
                    use base64::Engine;
                    let encode = base64::engine::general_purpose::STANDARD.encode(&octets);
                    let _ = db.set_setting(CLE_AVATAR, &format!("data:{type_};base64,{encode}"));
                    let _ = db.set_setting(CLE_AVATAR_ADRESSE, adresse);
                }
                Err(e) => log::warn!("compte : avatar illisible — {e}"),
            }
        }
        Ok(reponse) => log::warn!("compte : avatar indisponible ({})", reponse.status()),
        Err(e) => log::warn!("compte : avatar injoignable — {e}"),
    }
}

#[derive(Deserialize)]
struct Capacites {
    google: bool,
}

/// Ce que le serveur sait faire.
///
/// Rend `false` quand on ne peut pas lui demander : mieux vaut ne pas proposer un bouton que
/// d'en proposer un qui mene a une page ou le choix n'existe pas.
#[tauri::command]
pub async fn compte_google_disponible(
    state: tauri::State<'_, crate::AppState>,
) -> Result<bool, String> {
    let reponse = match client()
        .get(format!("{}/api/capacites", serveur(&state.db)))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            log::warn!("compte : capacites indisponibles ({})", r.status());
            return Ok(false);
        }
        Err(e) => {
            log::warn!("compte : capacites injoignables — {e}");
            return Ok(false);
        }
    };

    Ok(reponse.json::<Capacites>().await.map(|c| c.google).unwrap_or(false))
}

#[tauri::command]
pub async fn compte_etat(state: tauri::State<'_, crate::AppState>) -> Result<EtatCompte, String> {
    Ok(etat(&state.db))
}

#[tauri::command]
pub async fn compte_inscription(
    email: String,
    mot_de_passe: String,
    nom: Option<String>,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    ouvrir_une_session(&state.db, "/api/inscription", &email, &mot_de_passe, nom.as_deref()).await
}

#[tauri::command]
pub async fn compte_connexion(
    email: String,
    mot_de_passe: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    ouvrir_une_session(&state.db, "/api/connexion", &email, &mot_de_passe, None).await
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
            Ok(EtatAppairage::Accorde { compte: enregistrer_la_session(&state.db, &recue).await? })
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
    db.set_setting(CLE_INITIALES, "")?;
    db.set_setting(CLE_AVATAR, "")?;
    Ok(etat(db))
}

/// Appelle l'API du compte avec le jeton, et range le profil rendu.
///
/// Publique parce que les commandes Tauri ne s'essaient pas : elles demandent un `State` et une
/// fenetre. Les commandes ci-dessous ne sont que des adaptateurs vers ces fonctions-ci, qui
/// prennent une base et rien d'autre.
/// Relit le profil tel que le serveur le connait : nom, initiales, image.
///
/// Sans ca, ces trois choses n'arrivaient qu'a la CONNEXION. Changez votre image sur une
/// machine, l'autre — deja connectee — gardait l'ancienne pour toujours. Appele a chaque
/// passage de synchronisation, ou l'image ne se retelecharge que si son adresse a change.
pub async fn relire_le_profil(db: &Database) -> Result<EtatCompte, String> {
    appeler_le_profil(db, reqwest::Method::GET, "/api/moi", None, None).await
}

pub async fn appeler_le_profil(
    db: &Database,
    methode: reqwest::Method,
    chemin: &str,
    corps: Option<serde_json::Value>,
    image: Option<Vec<u8>>,
) -> Result<EtatCompte, String> {
    let Some(jeton) = jeton(db) else {
        return Err(motif::PAS_CONNECTE.to_string());
    };

    let mut demande = client()
        .request(methode, format!("{}{chemin}", serveur(db)))
        .bearer_auth(jeton);
    if let Some(corps) = corps {
        demande = demande.json(&corps);
    }
    if let Some(image) = image {
        demande = demande
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .body(image);
    }

    let reponse = demande.send().await.map_err(panne_reseau)?;
    if !reponse.status().is_success() {
        return Err(refus(reponse).await);
    }

    let recue: ReponseProfil = reponse.json().await.map_err(|e| {
        log::warn!("compte : profil illisible — {e}");
        motif::SERVEUR.to_string()
    })?;

    rapatrier_l_avatar(db, recue.compte.avatar.as_deref()).await;
    enregistrer_le_profil(db, &recue.compte)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub id: String,
    pub nom: String,
    pub systeme: String,
    pub vu_le: String,
}

#[derive(Deserialize)]
struct ReponseMoi {
    appareils: Vec<Machine>,
    /// L'identifiant de la machine COURANTE, pour que l'interface puisse la distinguer des
    /// autres — deconnecter celle sur laquelle on est n'a pas le meme sens.
    appareil: Option<String>,
}

/// La liste des machines du compte, et laquelle est celle-ci.
#[tauri::command]
pub async fn compte_machines(
    state: tauri::State<'_, crate::AppState>,
) -> Result<(Vec<Machine>, Option<String>), String> {
    let db = &state.db;
    let Some(jeton) = jeton(db) else {
        return Err(motif::PAS_CONNECTE.to_string());
    };

    let reponse = client()
        .get(format!("{}/api/moi", serveur(db)))
        .bearer_auth(jeton)
        .send()
        .await
        .map_err(panne_reseau)?;

    if !reponse.status().is_success() {
        return Err(refus(reponse).await);
    }

    let recue: ReponseMoi = reponse.json().await.map_err(|e| {
        log::warn!("compte : liste des machines illisible — {e}");
        motif::SERVEUR.to_string()
    })?;

    Ok((recue.appareils, recue.appareil))
}

pub async fn definir_le_nom(db: &Database, nom: &str) -> Result<EtatCompte, String> {
    appeler_le_profil(
        db,
        reqwest::Method::PATCH,
        "/api/moi",
        Some(serde_json::json!({ "nom": nom })),
        None,
    )
    .await
}

#[tauri::command]
pub async fn compte_definir_nom(
    nom: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    definir_le_nom(&state.db, &nom).await
}

/// Ce qu'on accepte de lire pour un avatar. Meme borne que le serveur : refuser ici evite de
/// faire traverser des mega-octets a l'interface pour qu'elle les refuse ensuite.
pub const AVATAR_TAILLE_MAXIMALE: usize = 4 * 1024 * 1024;

/// Reconnait le type d'une image A SES OCTETS, jamais a son extension : un fichier renomme
/// nous ferait sinon annoncer un type faux, et c'est le type annonce qui decide du decodage
/// cote interface comme cote serveur.
fn type_d_image(octets: &[u8]) -> Option<&'static str> {
    match octets {
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Some("image/jpeg"),
        [b'G', b'I', b'F', b'8', ..] => Some("image/gif"),
        // WEBP : « RIFF » puis quatre octets de taille, puis « WEBP ».
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("image/webp"),
        _ => None,
    }
}

/// Lit une image du disque et la rend en `data:` URL.
///
/// Sert au RECADRAGE : l'interface doit pouvoir montrer l'image avant de l'envoyer. C'est le
/// seul cas ou une image traverse l'IPC dans ce sens, et c'est pour cela que la taille est
/// bornee ici — un JPEG d'appareil photo fait plusieurs dizaines de mega-octets.
pub fn lire_une_image(chemin: &str) -> Result<String, String> {
    let octets = std::fs::read(chemin).map_err(|e| {
        log::warn!("compte : image illisible ({chemin}) — {e}");
        "avatar_illisible".to_string()
    })?;
    if octets.len() > AVATAR_TAILLE_MAXIMALE {
        return Err("avatar_trop_gros".to_string());
    }
    let type_ = type_d_image(&octets).ok_or_else(|| "avatar_format_refuse".to_string())?;

    use base64::Engine;
    let encode = base64::engine::general_purpose::STANDARD.encode(&octets);
    Ok(format!("data:{type_};base64,{encode}"))
}

/// Envoie des octets d'image au serveur, qui les reencode avant de les garder.
pub async fn deposer_une_image(db: &Database, octets: Vec<u8>) -> Result<EtatCompte, String> {
    if octets.is_empty() {
        return Err("avatar_vide".to_string());
    }
    if octets.len() > AVATAR_TAILLE_MAXIMALE {
        return Err("avatar_trop_gros".to_string());
    }
    appeler_le_profil(db, reqwest::Method::PUT, "/api/moi/avatar", None, Some(octets)).await
}

/// Depose une image choisie sur le disque, telle quelle.
///
/// Le fichier est lu ICI et non cote interface : passer une image par l'IPC la ferait encoder
/// en JavaScript puis reevaluer, pour rien. Le chemin du recadrage, lui, n'a pas le choix.
pub async fn deposer_un_avatar(db: &Database, chemin: &str) -> Result<EtatCompte, String> {
    let image = std::fs::read(chemin).map_err(|e| {
        log::warn!("compte : image illisible ({chemin}) — {e}");
        "avatar_illisible".to_string()
    })?;

    deposer_une_image(db, image).await
}

/// Decode une `data:` URL produite par l'interface (le resultat du recadrage).
pub fn octets_d_une_data_url(donnees: &str) -> Result<Vec<u8>, String> {
    let base = donnees
        .split_once(";base64,")
        .map(|(_, b)| b)
        .ok_or_else(|| "avatar_format_refuse".to_string())?;

    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(base)
        .map_err(|_| "avatar_illisible".to_string())
}

pub async fn retirer_l_avatar(db: &Database) -> Result<EtatCompte, String> {
    appeler_le_profil(db, reqwest::Method::DELETE, "/api/moi/avatar", None, None).await
}

#[tauri::command]
pub async fn compte_deposer_avatar(
    chemin: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    deposer_un_avatar(&state.db, &chemin).await
}

#[tauri::command]
pub fn compte_lire_image(chemin: String) -> Result<String, String> {
    lire_une_image(&chemin)
}

/// Depose l'image RECADREE par l'interface. Elle arrive en `data:` URL parce que c'est ce que
/// produit un canvas, et que le convertir en binaire cote interface ne gagnerait rien.
#[tauri::command]
pub async fn compte_deposer_image(
    donnees: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    let octets = octets_d_une_data_url(&donnees)?;
    deposer_une_image(&state.db, octets).await
}

#[tauri::command]
pub async fn compte_retirer_avatar(
    state: tauri::State<'_, crate::AppState>,
) -> Result<EtatCompte, String> {
    retirer_l_avatar(&state.db).await
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

    /// Un fichier renomme ne doit pas nous faire annoncer un type faux : c'est le type annonce
    /// qui decide du decodage, ici comme sur le serveur.
    #[test]
    fn le_type_d_image_se_lit_sur_les_octets() {
        assert_eq!(type_d_image(b"\x89PNG\r\n\x1a\n reste"), Some("image/png"));
        assert_eq!(type_d_image(b"\xFF\xD8\xFF\xE0 reste"), Some("image/jpeg"));
        assert_eq!(type_d_image(b"GIF89a reste"), Some("image/gif"));
        assert_eq!(type_d_image(b"RIFF\x20\x00\x00\x00WEBPVP8 "), Some("image/webp"));
        // Un RIFF qui n'est pas du WEBP (un WAV, par exemple) est refuse.
        assert_eq!(type_d_image(b"RIFF\x20\x00\x00\x00WAVEfmt "), None);
        assert_eq!(type_d_image(b"<html>"), None);
        assert_eq!(type_d_image(b""), None);
    }

    #[test]
    fn une_image_absente_ou_trop_grosse_est_refusee() {
        let dossier = std::env::temp_dir().join(format!("ckpt-av-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();

        assert_eq!(
            lire_une_image(&dossier.join("rien.png").to_string_lossy()),
            Err("avatar_illisible".to_string())
        );

        let gros = dossier.join("gros.png");
        std::fs::write(&gros, vec![0u8; AVATAR_TAILLE_MAXIMALE + 1]).unwrap();
        assert_eq!(
            lire_une_image(&gros.to_string_lossy()),
            Err("avatar_trop_gros".to_string())
        );

        // Sous la borne mais pas une image : refuse sur le TYPE, pas sur la taille.
        let faux = dossier.join("faux.png");
        std::fs::write(&faux, b"pas une image").unwrap();
        assert_eq!(
            lire_une_image(&faux.to_string_lossy()),
            Err("avatar_format_refuse".to_string())
        );

        let vrai = dossier.join("vrai.png");
        std::fs::write(&vrai, b"\x89PNG\r\n\x1a\nsuite").unwrap();
        let rendu = lire_une_image(&vrai.to_string_lossy()).unwrap();
        assert!(rendu.starts_with("data:image/png;base64,"), "rendu : {rendu}");

        std::fs::remove_dir_all(&dossier).ok();
    }

    #[test]
    fn une_data_url_se_decode_et_un_texte_nu_est_refuse() {
        assert_eq!(octets_d_une_data_url("data:image/png;base64,QUJD").unwrap(), b"ABC");
        // Sans le prefixe, on ne devine pas : on refuse.
        assert_eq!(
            octets_d_une_data_url("QUJD"),
            Err("avatar_format_refuse".to_string())
        );
        assert_eq!(
            octets_d_une_data_url("data:image/png;base64,ce n'est pas du base64"),
            Err("avatar_illisible".to_string())
        );
    }

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
