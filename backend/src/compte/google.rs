//! La connexion Google, sans code a comparer.
//!
//! # Ce que ce fichier remplace
//!
//! Le detour par le navigateur avec un code affiche des deux cotes venait d'une regle qu'on
//! s'etait donnee : « Cockpit n'a pas de serveur HTTP ». Elle etait presentee comme une
//! contrainte, c'etait un choix — et il coutait cher : l'utilisateur voyait un code, croyait
//! devoir le recopier, et ne comprenait pas ce qu'on lui demandait.
//!
//! La facon dont Google recommande de faire pour une application de bureau est un ecouteur sur la
//! BOUCLE LOCALE, ouvert le temps de la connexion. Ce n'est pas un serveur : rien de persistant,
//! aucun port expose au-dela de `127.0.0.1`, et il se ferme des que le navigateur a repondu.
//!
//! # Ce qui protege l'echange
//!
//! - **PKCE.** Le verificateur ne quitte jamais ce processus ; seule son empreinte part dans
//!   l'adresse. Un code intercepte est donc inutilisable sans lui. C'est ce qui rend acceptable
//!   qu'un secret de client soit embarque dans un logiciel distribue — Google le dit lui-meme,
//!   ce n'est pas un secret, et c'est pour ca qu'il ne suffit pas.
//! - **L'etat.** Une reponse dont l'etat ne correspond pas est jetee : sans ce controle,
//!   n'importe quelle page pourrait faire aboutir une connexion qu'on n'a pas demandee.
//! - **La boucle locale seulement.** L'ecouteur est lie a `127.0.0.1` et non a `0.0.0.0` : rien
//!   d'autre sur le reseau ne peut l'atteindre.

use base64::Engine;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

/// Le client OAuth de type « application de bureau », pose au BUILD.
///
/// Vide en developpement : la commande refuse alors clairement au lieu d'ouvrir un navigateur sur
/// une page d'erreur de Google.
const CLIENT_ID: &str = match option_env!("COCKPIT_GOOGLE_ID") {
    Some(v) => v,
    None => "",
};
const CLIENT_SECRET: &str = match option_env!("COCKPIT_GOOGLE_SECRET") {
    Some(v) => v,
    None => "",
};

/// Trois minutes : le temps de choisir un compte, pas celui d'oublier la fenetre ouverte.
const DELAI: Duration = Duration::from_secs(180);

pub fn configure() -> bool {
    !CLIENT_ID.is_empty() && !CLIENT_SECRET.is_empty()
}

/// Ce qu'une connexion reussie rapporte : le jeton d'identite, a presenter a NOTRE serveur.
pub struct Identite {
    pub jeton_identite: String,
}

/// Les trois valeurs tirees au hasard pour une connexion.
struct Secrets {
    verificateur: String,
    empreinte: String,
    etat: String,
}

impl Secrets {
    fn tirer() -> Result<Self, String> {
        let verificateur = alea(32)?;

        Ok(Self { empreinte: empreinte(&verificateur), verificateur, etat: alea(16)? })
    }
}

/// L'empreinte PKCE d'un verificateur : base64 sans remplissage sur son SHA-256.
///
/// Fonction a part pour etre ESSAYABLE. Un essai qui recalcule l'empreinte de son cote ne
/// verifie rien : il passe meme quand le code, lui, ne hache plus rien — constate le 2026-08-24.
fn empreinte(verificateur: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verificateur.as_bytes()))
}

fn alea(octets: usize) -> Result<String, String> {
    let mut brut = vec![0u8; octets];
    getrandom::getrandom(&mut brut).map_err(|e| {
        log::error!("google : alea indisponible — {e}");
        "alea_indisponible".to_string()
    })?;

    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(brut))
}

/// Mene la connexion de bout en bout et rend le jeton d'identite.
///
/// `ouvrir` recoit l'adresse a ouvrir dans le navigateur du systeme. Elle est passee en parametre
/// plutot qu'appelee ici pour que le chemin soit essayable sans navigateur.
pub async fn obtenir_une_identite(
    ouvrir: impl FnOnce(&str) -> Result<(), String>,
) -> Result<Identite, String> {
    if !configure() {
        log::warn!("google : aucun client de bureau pose au build");
        return Err("google_non_configure".to_string());
    }

    // Port 0 : le systeme en choisit un libre. Le fixer serait un conflit garanti le jour ou deux
    // Cockpit tournent, ou ou un autre programme l'occupe.
    let ecouteur = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .map_err(|e| {
            log::error!("google : impossible d'ecouter sur la boucle locale — {e}");
            "ecoute_impossible".to_string()
        })?;
    let port = ecouteur.local_addr().map_err(|_| "ecoute_impossible".to_string())?.port();
    let redirection = format!("http://127.0.0.1:{port}");

    let secrets = Secrets::tirer()?;
    ouvrir(&adresse_d_autorisation(&secrets, &redirection))?;

    let (code, etat_recu) = attendre_la_reponse(ecouteur).await?;

    // L'etat d'abord : on ne presente pas a Google un code dont on ne sait pas s'il repond a
    // notre demande.
    if etat_recu != secrets.etat {
        log::warn!("google : etat inattendu, reponse jetee");
        return Err("reponse_inattendue".to_string());
    }

    let jeton_identite = echanger_le_code(&code, &secrets.verificateur, &redirection).await?;

    Ok(Identite { jeton_identite })
}

fn adresse_d_autorisation(secrets: &Secrets, redirection: &str) -> String {
    let parametres = [
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirection),
        ("response_type", "code"),
        ("scope", "openid email profile"),
        ("code_challenge", &secrets.empreinte),
        ("code_challenge_method", "S256"),
        ("state", &secrets.etat),
        // Le compte est demande explicitement : sans ca, quelqu'un ayant plusieurs comptes
        // Google est connecte avec le dernier utilise, sans qu'on lui demande.
        ("prompt", "select_account"),
    ];

    let requete: Vec<String> = parametres
        .iter()
        .map(|(cle, valeur)| format!("{cle}={}", encoder(valeur)))
        .collect();

    format!("https://accounts.google.com/o/oauth2/v2/auth?{}", requete.join("&"))
}

/// Encodage pour une requete d'adresse. Ecrit ici plutot que tire d'une caisse : les caracteres a
/// laisser passer sont peu nombreux et la regle tient en trois lignes.
fn encoder(valeur: &str) -> String {
    valeur
        .bytes()
        .map(|o| match o {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (o as char).to_string()
            }
            _ => format!("%{o:02X}"),
        })
        .collect()
}

/// Attend que le navigateur revienne, au plus `DELAI`.
///
/// L'ecouteur est mis en NON BLOQUANT et interroge : un `accept` bloquant ne se laisse pas
/// interrompre, donc une connexion abandonnee laisserait un fil coince jusqu'a la fermeture de
/// l'application.
async fn attendre_la_reponse(ecouteur: TcpListener) -> Result<(String, String), String> {
    ecouteur.set_nonblocking(true).map_err(|_| "ecoute_impossible".to_string())?;
    let fin = Instant::now() + DELAI;

    loop {
        match ecouteur.accept() {
            Ok((flux, _)) => return lire_la_reponse(flux),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= fin {
                    log::info!("google : personne n'est revenu du navigateur");
                    return Err("delai_depasse".to_string());
                }
                tokio::time::sleep(Duration::from_millis(120)).await;
            }
            Err(e) => {
                log::error!("google : echec de l'ecoute — {e}");
                return Err("ecoute_impossible".to_string());
            }
        }
    }
}

fn lire_la_reponse(mut flux: TcpStream) -> Result<(String, String), String> {
    flux.set_read_timeout(Some(Duration::from_secs(5))).ok();

    // La premiere ligne suffit : « GET /?code=...&state=... HTTP/1.1 ». On borne la lecture — ce
    // qui arrive ici vient du reseau, meme local.
    let mut tampon = [0u8; 8192];
    let lus = flux.read(&mut tampon).map_err(|e| {
        log::warn!("google : reponse du navigateur illisible — {e}");
        "reponse_illisible".to_string()
    })?;
    let brut = String::from_utf8_lossy(&tampon[..lus]);

    let premiere = brut.lines().next().unwrap_or_default();
    let chemin = premiere.split_whitespace().nth(1).unwrap_or_default();
    let parametres = parametres_de(chemin);

    let page = |titre: &str| {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
             Connection: close\r\nContent-Length: {}\r\n\r\n{}",
            titre.len(),
            titre
        )
    };

    // Google renvoie `error` quand la personne refuse : ce n'est pas une panne, c'est un choix.
    if let Some(refus) = parametres.iter().find(|(c, _)| c == "error") {
        let _ = flux.write_all(page("Vous pouvez fermer cette fenetre.").as_bytes());
        log::info!("google : connexion refusee par l'utilisateur ({})", refus.1);
        return Err("refus_utilisateur".to_string());
    }

    let trouver = |nom: &str| {
        parametres.iter().find(|(c, _)| c == nom).map(|(_, v)| v.clone()).unwrap_or_default()
    };
    let (code, etat) = (trouver("code"), trouver("state"));

    let _ = flux.write_all(page("C'est fait. Vous pouvez fermer cette fenetre.").as_bytes());
    let _ = flux.flush();

    if code.is_empty() {
        return Err("reponse_illisible".to_string());
    }

    Ok((code, etat))
}

/// Les parametres d'un chemin de requete, decodes.
fn parametres_de(chemin: &str) -> Vec<(String, String)> {
    chemin
        .split_once('?')
        .map(|(_, requete)| requete)
        .unwrap_or_default()
        .split('&')
        .filter(|morceau| !morceau.is_empty())
        .filter_map(|morceau| {
            let (cle, valeur) = morceau.split_once('=')?;
            Some((cle.to_string(), decoder(valeur)))
        })
        .collect()
}

fn decoder(valeur: &str) -> String {
    let octets = valeur.as_bytes();
    let mut sortie = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        match octets[i] {
            b'%' if i + 2 < octets.len() => {
                match u8::from_str_radix(&valeur[i + 1..i + 3], 16) {
                    Ok(o) => {
                        sortie.push(o);
                        i += 3;
                    }
                    Err(_) => {
                        sortie.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                sortie.push(b' ');
                i += 1;
            }
            o => {
                sortie.push(o);
                i += 1;
            }
        }
    }

    String::from_utf8_lossy(&sortie).to_string()
}

#[derive(serde::Deserialize)]
struct ReponseJeton {
    id_token: Option<String>,
}

async fn echanger_le_code(
    code: &str,
    verificateur: &str,
    redirection: &str,
) -> Result<String, String> {
    let reponse = super::client()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", code),
            ("client_id", CLIENT_ID),
            ("client_secret", CLIENT_SECRET),
            ("code_verifier", verificateur),
            ("redirect_uri", redirection),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| {
            log::warn!("google : echange du code impossible — {e}");
            "reseau".to_string()
        })?;

    if !reponse.status().is_success() {
        log::warn!("google : echange du code refuse ({})", reponse.status());
        return Err("echange_refuse".to_string());
    }

    reponse
        .json::<ReponseJeton>()
        .await
        .ok()
        .and_then(|r| r.id_token)
        .filter(|j| !j.is_empty())
        .ok_or_else(|| {
            log::warn!("google : aucun jeton d'identite dans la reponse");
            "echange_refuse".to_string()
        })
}

#[cfg(test)]
mod tests {
    /// L'encodage doit laisser passer les caracteres non reserves et echapper le reste : une
    /// adresse mal encodee fait refuser la demande par Google, avec un message qui ne dit pas
    /// pourquoi.
    #[test]
    fn l_encodage_echappe_ce_qu_il_faut() {
        assert_eq!(super::encoder("openid email profile"), "openid%20email%20profile");
        assert_eq!(super::encoder("http://127.0.0.1:53682"), "http%3A%2F%2F127.0.0.1%3A53682");
        // Les non reserves passent tels quels : les echapper est valide mais illisible.
        assert_eq!(super::encoder("aZ09-_.~"), "aZ09-_.~");
    }

    #[test]
    fn les_parametres_sont_lus_et_decodes() {
        let lus = super::parametres_de("/?code=4%2F0Ab_x&state=abc&scope=openid+email");
        let trouver = |nom: &str| {
            lus.iter().find(|(c, _)| c == nom).map(|(_, v)| v.clone()).unwrap_or_default()
        };

        assert_eq!(trouver("code"), "4/0Ab_x");
        assert_eq!(trouver("state"), "abc");
        assert_eq!(trouver("scope"), "openid email");
    }

    /// Un chemin sans requete ne doit pas paniquer : le navigateur demande aussi `/favicon.ico`.
    #[test]
    fn un_chemin_sans_requete_ne_rend_rien() {
        assert!(super::parametres_de("/favicon.ico").is_empty());
        assert!(super::parametres_de("").is_empty());
    }

    /// L'empreinte PKCE est celle que Google attend. Le couple vient de la RFC 7636, et l'essai
    /// appelle NOTRE fonction : le recalculer de son cote ne verifiait rien — il passait meme
    /// avec le hachage retire.
    #[test]
    fn l_empreinte_pkce_suit_la_norme() {
        assert_eq!(
            super::empreinte("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        );
    }

    /// Et l'empreinte posee dans l'adresse est bien celle du verificateur tire : sans cet essai,
    /// on pourrait envoyer l'empreinte de quelque chose d'autre sans que rien ne le dise.
    #[test]
    fn l_empreinte_envoyee_correspond_au_verificateur() {
        let secrets = super::Secrets::tirer().expect("alea");

        assert_eq!(secrets.empreinte, super::empreinte(&secrets.verificateur));
        assert_ne!(secrets.empreinte, secrets.verificateur, "l'empreinte n'est pas le secret");
    }

    /// Deux connexions ne doivent jamais tirer les memes secrets, sinon l'etat ne protege plus
    /// de rien.
    #[test]
    fn chaque_connexion_tire_des_secrets_differents() {
        let a = super::Secrets::tirer().expect("alea");
        let b = super::Secrets::tirer().expect("alea");

        assert_ne!(a.verificateur, b.verificateur);
        assert_ne!(a.etat, b.etat);
        // 32 octets en base64 sans remplissage : 43 caracteres, dans les bornes de la RFC.
        assert_eq!(a.verificateur.len(), 43);
    }
}
