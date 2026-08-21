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
