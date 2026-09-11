//! Le profil du compte, du logiciel jusqu'au serveur.
//!
//! Ce que les essais unitaires ne peuvent pas montrer : que ce que le logiciel ENVOIE
//! correspond a ce que le serveur ATTEND. Un nom de champ qui differe d'un cote et de l'autre
//! passe toutes les verifications separees et ne casse qu'a l'usage. Et le depot d'un avatar
//! demande, par l'interface, de passer par le selecteur de fichier du bureau : essayable a la
//! main quand un portail repond, mais pas de facon reproductible.
//!
//! Marque `ignore` : demande un serveur en marche. Voir `synchro_bout_en_bout.rs` pour la
//! recette, et **un serveur fraichement demarre** — les tentatives de connexion sont comptees
//! par adresse.
//!
//! ```text
//! COCKPIT_SERVEUR_ESSAI=http://127.0.0.1:8094 \
//!     cargo test --test profil_bout_en_bout -- --ignored --test-threads=1
//! ```

use cockpit_lib::storage::db::Database;

fn adresse() -> String {
    std::env::var("COCKPIT_SERVEUR_ESSAI")
        .expect("COCKPIT_SERVEUR_ESSAI doit designer un serveur en marche")
}

/// Une base neuve avec un compte tout juste cree et son jeton range.
async fn poste(email: &str) -> Database {
    let db = Database::new(":memory:").unwrap();
    let adresse = adresse();

    let reponse = reqwest::Client::new()
        .post(format!("{adresse}/api/inscription"))
        .json(&serde_json::json!({
            "email": email,
            "mot_de_passe": "motdepasse-assez-long",
            "appareil": {
                "cle": uuid::Uuid::new_v4().to_string(),
                "nom": "poste-essai",
                "systeme": "linux",
            },
        }))
        .send()
        .await
        .expect("le serveur d'essai doit repondre");
    assert!(
        reponse.status().is_success(),
        "inscription refusee ({}) — un serveur fraichement demarre est necessaire",
        reponse.status(),
    );

    let recue: serde_json::Value = reponse.json().await.unwrap();
    db.set_setting("compte_jeton", recue["jeton"].as_str().unwrap()).unwrap();
    db.set_setting("compte_serveur", &adresse).unwrap();
    db
}

/// Un PNG minuscule, fabrique ici : dependre d'un fichier du depot ferait echouer l'essai le
/// jour ou quelqu'un le deplace.
fn png(cote: u32) -> Vec<u8> {
    let mut lignes = Vec::new();
    for _ in 0..cote {
        lignes.push(0u8);
        for _ in 0..cote {
            lignes.extend_from_slice(&[220, 90, 40]);
        }
    }

    fn bloc(type_: &[u8], donnees: &[u8]) -> Vec<u8> {
        let mut sortie = (donnees.len() as u32).to_be_bytes().to_vec();
        let corps: Vec<u8> = type_.iter().chain(donnees).copied().collect();
        sortie.extend_from_slice(&corps);
        sortie.extend_from_slice(&crc32(&corps).to_be_bytes());
        sortie
    }

    let mut entete = Vec::new();
    entete.extend_from_slice(&cote.to_be_bytes());
    entete.extend_from_slice(&cote.to_be_bytes());
    entete.extend_from_slice(&[8, 2, 0, 0, 0]);

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(bloc(b"IHDR", &entete));
    png.extend(bloc(b"IDAT", &compresser(&lignes)));
    png.extend(bloc(b"IEND", b""));
    png
}

/// Un flux zlib « sans compression » : suffisant, et evite une dependance de plus.
fn compresser(donnees: &[u8]) -> Vec<u8> {
    let mut sortie = vec![0x78, 0x01];
    for (i, morceau) in donnees.chunks(65_535).enumerate() {
        let dernier = (i + 1) * 65_535 >= donnees.len();
        sortie.push(u8::from(dernier));
        sortie.extend_from_slice(&(morceau.len() as u16).to_le_bytes());
        sortie.extend_from_slice(&(!(morceau.len() as u16)).to_le_bytes());
        sortie.extend_from_slice(morceau);
    }
    sortie.extend_from_slice(&adler32(donnees).to_be_bytes());
    sortie
}

fn adler32(donnees: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for octet in donnees {
        a = (a + u32::from(*octet)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn crc32(donnees: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for octet in donnees {
        crc ^= u32::from(*octet);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn changer_son_nom_change_ses_initiales() {
    let db = poste(&format!("nom-{}@exemple.test", uuid::Uuid::new_v4().simple())).await;

    let etat = cockpit_lib::compte::definir_le_nom(&db, "Camille Martin")
        .await
        .expect("le serveur doit accepter le nom");

    assert_eq!(etat.nom.as_deref(), Some("Camille Martin"));
    assert_eq!(etat.initiales.as_deref(), Some("CM"));
}

/// Le chemin qui, par l'interface, demande le selecteur de fichier du bureau : essayable a la
/// main quand un portail repond, mais pas de facon reproductible. D'ou cet essai.
#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn deposer_une_image_la_garde_en_local_pour_l_afficher_hors_ligne() {
    let db = poste(&format!("img-{}@exemple.test", uuid::Uuid::new_v4().simple())).await;

    let fichier = std::env::temp_dir().join(format!("cockpit-essai-{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&fichier, png(120)).unwrap();

    let etat = cockpit_lib::compte::deposer_un_avatar(&db, fichier.to_str().unwrap())
        .await
        .expect("le serveur doit accepter l'image");

    let avatar = etat.avatar.expect("le portrait doit etre garde en local");
    assert!(
        avatar.starts_with("data:image/"),
        "l'image doit etre gardee en adresse `data:` pour s'afficher hors ligne : {}",
        &avatar[..avatar.len().min(40)],
    );

    let vide = cockpit_lib::compte::retirer_l_avatar(&db).await.unwrap();
    assert!(vide.avatar.is_none(), "retirer l'image doit aussi vider ce qui est garde ici");

    std::fs::remove_file(&fichier).ok();
}

/// L'autre chemin de depot : l'image RECADREE par l'interface, qui arrive en `data:` URL parce
/// que c'est ce que produit un canvas. Ce que le logiciel envoie doit correspondre a ce que le
/// serveur attend, et un decodage rate ne se verrait qu'a l'usage.
#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn une_image_recadree_arrive_aussi_bien_qu_un_fichier() {
    let db = poste(&format!("cadre-{}@exemple.test", uuid::Uuid::new_v4().simple())).await;

    use base64::Engine;
    let encode = base64::engine::general_purpose::STANDARD.encode(png(200));
    let url = format!("data:image/png;base64,{encode}");

    let octets = cockpit_lib::compte::octets_d_une_data_url(&url).expect("decodage");
    let etat = cockpit_lib::compte::deposer_une_image(&db, octets)
        .await
        .expect("le serveur doit accepter l'image recadree");

    assert!(
        etat.avatar.is_some_and(|a| a.starts_with("data:image/")),
        "le portrait doit etre garde en local, comme pour un fichier"
    );
}

/// Une image vide n'atteint jamais le serveur : la refuser ici evite un aller-retour et un
/// message technique venu d'ailleurs.
#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn une_image_vide_est_refusee_avant_le_reseau() {
    let db = poste(&format!("vide-{}@exemple.test", uuid::Uuid::new_v4().simple())).await;

    assert_eq!(
        cockpit_lib::compte::deposer_une_image(&db, Vec::new()).await.unwrap_err(),
        "avatar_vide"
    );
}

#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn un_fichier_qui_n_est_pas_une_image_est_refuse_avec_un_motif() {
    let db = poste(&format!("faux-{}@exemple.test", uuid::Uuid::new_v4().simple())).await;

    let fichier = std::env::temp_dir().join(format!("cockpit-faux-{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&fichier, b"<?php echo 'bonjour';").unwrap();

    let refus = cockpit_lib::compte::deposer_un_avatar(&db, fichier.to_str().unwrap())
        .await
        .expect_err("un fichier qui n'est pas une image doit etre refuse");

    assert_eq!(
        refus, "avatar_format_refuse",
        "le motif doit etre celui du serveur, pour que l'interface sache quoi dire",
    );

    std::fs::remove_file(&fichier).ok();
}
