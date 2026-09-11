//! Deux machines, un vrai serveur : ce que les essais unitaires ne peuvent pas montrer.
//!
//! Chaque piece est verifiee de son cote — le journal, l'application, l'API du serveur. Leur
//! RENCONTRE ne l'est nulle part ailleurs : un nom de champ qui differe d'un cote et de
//! l'autre passe toutes les verifications separees et ne casse qu'a l'usage.
//!
//! Marque `ignore` parce qu'il demande un serveur en marche. Pour le lancer :
//!
//! ```text
//! docker run -d --name base-synchro -e POSTGRES_USER=app -e POSTGRES_PASSWORD=x \
//!     -e POSTGRES_DB=app postgres:17-alpine
//! # puis le serveur, avec DATABASE_URL pointant dessus, sur le port 8097
//! COCKPIT_SERVEUR_ESSAI=http://127.0.0.1:8097 \
//!     cargo test --test synchro_bout_en_bout -- --ignored --test-threads=1
//! ```
//!
//! **Le serveur doit etre FRAICHEMENT demarre.** Il compte les tentatives de connexion par
//! adresse, et toute la suite vient de la meme : relancee deux fois de suite sur le meme
//! conteneur, elle se refuse elle-meme. Un echec de connexion ici veut dire ca, pas un defaut.

use cockpit_lib::storage::db::Database;

fn adresse() -> String {
    std::env::var("COCKPIT_SERVEUR_ESSAI")
        .expect("COCKPIT_SERVEUR_ESSAI doit designer un serveur en marche")
}

/// Prepare une machine : base neuve, compte cree ou rejoint, jeton range.
///
/// `creer` dit lequel des deux : tenter la creation puis se rabattre couterait un appel de
/// plus, et le serveur compte les tentatives par adresse — la suite s'epuiserait elle-meme.
/// Une image minuscule mais VALIDE : le serveur la decode vraiment avant de la garder, donc un
/// tableau d'octets au hasard serait refuse. Huit pixels de cote suffisent.
const PNG_ESSAI: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 8, 0, 0, 0, 8, 8, 2,
    0, 0, 0, 75, 109, 41, 220, 0, 0, 0, 20, 73, 68, 65, 84, 120, 156, 99, 188, 19, 165, 193,
    128, 13, 48, 97, 21, 29, 180, 18, 0, 22, 127, 1, 110, 204, 247, 28, 110, 0, 0, 0, 0, 73, 69,
    78, 68, 174, 66, 96, 130
];

async fn machine(email: &str, nom: &str, creer: bool) -> Database {
    let db = Database::new(":memory:").unwrap();
    let adresse = adresse();
    let client = reqwest::Client::new();

    let corps = serde_json::json!({
        "email": email,
        "mot_de_passe": "motdepasse-assez-long",
        "appareil": {
            "cle": uuid::Uuid::new_v4().to_string(),
            "nom": nom,
            "systeme": "linux",
        },
    });

    let chemin = if creer { "/api/inscription" } else { "/api/connexion" };
    let reponse = client
        .post(format!("{adresse}{chemin}"))
        .json(&corps)
        .send()
        .await
        .expect("le serveur d'essai doit repondre");
    assert!(
        reponse.status().is_success(),
        "{chemin} a rendu {} — un serveur fraichement demarre est necessaire, sinon la limite \
         de tentatives par adresse refuse la suite",
        reponse.status(),
    );

    let recue: serde_json::Value = reponse.json().await.unwrap();
    db.set_setting("compte_jeton", recue["jeton"].as_str().unwrap()).unwrap();
    db.set_setting("compte_serveur", &adresse).unwrap();
    db
}

/// LE DEFAUT QUI A FAIT CROIRE QUE LA SYNCHRONISATION NE MARCHAIT PAS.
///
/// Une machine qui a servi AVANT que la synchronisation existe a une identite pour chaque
/// element et un journal vide : les declencheurs ne voient que ce qui change, et rien n'a
/// change. Elle n'envoyait donc jamais son contenu, et la seconde machine ne recevait rien —
/// pour toujours. Constate sur une vraie installation : 248 identites, 0 en file, 0 au serveur.
#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn le_contenu_d_avant_le_compte_arrive_sur_l_autre_machine() {
    let email = format!("avant-{}@exemple.test", uuid::Uuid::new_v4().simple());

    // ── Machine A, telle qu'elle etait avant cette fonctionnalite : des donnees, pas de
    // declencheurs, et le drapeau d'amorcage absent.
    let a = machine(&email, "poste-a", true).await;
    {
        let conn = a.conn();
        let noms: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'")
            .unwrap()
            .query_map([], |l| l.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for nom in noms {
            conn.execute_batch(&format!("DROP TRIGGER {nom}")).unwrap();
        }
        conn.execute(
            "INSERT INTO projects (name, path) VALUES ('projet-d-avant', '/tmp/avant')",
            [],
        )
        .unwrap();
    }
    a.set_setting("sync_journal_amorce", "").unwrap();
    a.conn()
        .execute("DELETE FROM settings WHERE key = 'sync_journal_amorce'", [])
        .unwrap();

    // Le demarrage suivant repose les declencheurs ET met le contenu en file.
    a.preparer_la_synchro().unwrap();
    assert_eq!(
        a.changements_en_attente().unwrap().len(),
        1,
        "le contenu d'avant doit etre en file au demarrage"
    );

    let envoi = cockpit_lib::compte::synchro::passer(&a).await.unwrap();
    assert_eq!(envoi.envoyes, 1, "il devait partir");

    // ── Machine B, neuve, meme compte.
    let b = machine(&email, "poste-b", false).await;
    let recu = cockpit_lib::compte::synchro::passer(&b).await.unwrap();
    assert!(recu.recus >= 1, "la machine neuve doit recevoir le projet d'avant");

    let noms: Vec<String> = b
        .conn()
        .prepare("SELECT name FROM projects")
        .unwrap()
        .query_map([], |l| l.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        noms.iter().any(|n| n == "projet-d-avant"),
        "le projet d'avant devait arriver ; recu : {noms:?}"
    );
}

/// L'image de profil ne voyage pas par le journal : elle vit sur le serveur. Elle n'arrivait
/// donc qu'a la CONNEXION — changez-la sur une machine, l'autre gardait l'ancienne pour
/// toujours. C'est le passage de synchronisation qui relit le profil.
#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn une_image_changee_ailleurs_arrive_sans_se_reconnecter() {
    let email = format!("img-{}@exemple.test", uuid::Uuid::new_v4().simple());

    let a = machine(&email, "poste-a", true).await;
    let b = machine(&email, "poste-b", false).await;

    // B n'a pas d'image au depart.
    assert!(
        b.get_setting("compte_avatar").unwrap_or_default().is_empty(),
        "aucune image ne devrait etre rangee avant d'en poser une"
    );

    // A pose une image.
    cockpit_lib::compte::deposer_une_image(&a, PNG_ESSAI.to_vec()).await.expect("depot");
    let sur_a = a.get_setting("compte_avatar").unwrap_or_default();
    assert!(sur_a.starts_with("data:image/"), "A doit avoir son image");

    // B ne se reconnecte pas : un simple passage suffit.
    cockpit_lib::compte::synchro::passer(&b).await.expect("passage");
    let sur_b = b.get_setting("compte_avatar").unwrap_or_default();
    assert!(sur_b.starts_with("data:image/"), "B devait recevoir l'image");
    assert_eq!(sur_a, sur_b, "les deux machines doivent montrer la MEME image");

    // Un second passage ne retelecharge rien : l'adresse porte la version, elle n'a pas bouge.
    let adresse_avant = b.get_setting("compte_avatar_adresse").unwrap_or_default();
    assert!(!adresse_avant.is_empty(), "l'adresse vue doit etre gardee");
    cockpit_lib::compte::synchro::passer(&b).await.expect("second passage");
    assert_eq!(b.get_setting("compte_avatar_adresse").unwrap_or_default(), adresse_avant);

    // A retire l'image : B la perd aussi, toujours sans se reconnecter.
    cockpit_lib::compte::retirer_l_avatar(&a).await.expect("retrait");
    cockpit_lib::compte::synchro::passer(&b).await.expect("passage");
    assert!(
        b.get_setting("compte_avatar").unwrap_or_default().is_empty(),
        "le retrait doit se voir aussi"
    );
}

#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn une_tache_creee_sur_une_machine_arrive_sur_l_autre() {
    let email = format!("bout-{}@exemple.test", uuid::Uuid::new_v4().simple());
    let a = machine(&email, "poste-a", true).await;
    let b = machine(&email, "poste-b", false).await;

    a.conn()
        .execute(
            "INSERT INTO todos (project, text, done, position) VALUES ('demo', 'ecrire la doc', 1, 0)",
            [],
        )
        .unwrap();

    let envoi = cockpit_lib::compte::synchro::passer(&a).await.expect("l'envoi doit aboutir");
    assert_eq!(envoi.envoyes, 1, "la tache creee doit partir");
    assert_eq!(envoi.recus, 0, "une machine ne recoit pas son propre envoi");

    let reception = cockpit_lib::compte::synchro::passer(&b).await.expect("la reception doit aboutir");
    assert_eq!(reception.recus, 1);

    let (texte, faite): (String, i64) = b
        .conn()
        .query_row("SELECT text, done FROM todos", [], |l| Ok((l.get(0)?, l.get(1)?)))
        .expect("la tache doit exister sur la seconde machine");
    assert_eq!(texte, "ecrire la doc");
    assert_eq!(faite, 1, "une tache cochee doit arriver cochee");

    // Rappeler ne doit rien redonner : sinon le logiciel retelechargerait tout a chaque passage.
    let encore = cockpit_lib::compte::synchro::passer(&b).await.unwrap();
    assert_eq!(encore.recus, 0);
}

#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn une_suppression_faite_sur_une_machine_efface_sur_l_autre() {
    let email = format!("suppr-{}@exemple.test", uuid::Uuid::new_v4().simple());
    let a = machine(&email, "poste-a", true).await;
    let b = machine(&email, "poste-b", false).await;

    a.conn()
        .execute("INSERT INTO urls (project, label, url) VALUES ('demo', 'Site', 'https://x.test')", [])
        .unwrap();
    cockpit_lib::compte::synchro::passer(&a).await.unwrap();
    cockpit_lib::compte::synchro::passer(&b).await.unwrap();
    assert_eq!(
        b.conn().query_row::<i64, _, _>("SELECT COUNT(*) FROM urls", [], |l| l.get(0)).unwrap(),
        1,
    );

    a.conn().execute("DELETE FROM urls", []).unwrap();
    cockpit_lib::compte::synchro::passer(&a).await.unwrap();
    cockpit_lib::compte::synchro::passer(&b).await.unwrap();

    assert_eq!(
        b.conn().query_row::<i64, _, _>("SELECT COUNT(*) FROM urls", [], |l| l.get(0)).unwrap(),
        0,
        "sans trace de suppression, l'element reviendrait depuis l'autre machine au tour suivant",
    );
}

#[tokio::test]
#[ignore = "demande un serveur en marche : voir l'en-tete du fichier"]
async fn le_dossier_d_un_projet_ne_traverse_pas() {
    let email = format!("chemin-{}@exemple.test", uuid::Uuid::new_v4().simple());
    let a = machine(&email, "poste-a", true).await;
    let b = machine(&email, "poste-b", false).await;

    a.conn()
        .execute("INSERT INTO projects (name, path) VALUES ('demo', '/home/moi/demo')", [])
        .unwrap();
    cockpit_lib::compte::synchro::passer(&a).await.unwrap();
    cockpit_lib::compte::synchro::passer(&b).await.unwrap();

    let (nom, chemin): (String, String) = b
        .conn()
        .query_row("SELECT name, path FROM projects", [], |l| Ok((l.get(0)?, l.get(1)?)))
        .expect("le projet doit arriver");
    assert_eq!(nom, "demo");
    assert_eq!(
        chemin, "",
        "un chemin d'une autre machine ne veut rien dire ici : il ferait pointer les onglets \
         vers un dossier absent",
    );
}
