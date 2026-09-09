//! Servir les commandes du backend hors de Tauri, sur un tuyau de lignes JSON.
//!
//! **POURQUOI CE MODULE EXISTE.** Le journal du guetteur compte 1 143 episodes ou la page
//! parle encore et ne peint plus, contre 4 ou notre propre boucle bloquait : le defaut est
//! dans le moteur de rendu de la vue web, pas dans ce backend. Le remplacer suppose un hote
//! qui n'est pas Tauri, donc un chemin pour lui servir les memes commandes.
//!
//! **LE PROTOCOLE EST DELIBEREMENT PAUVRE** : une ligne de JSON par message, dans les deux
//! sens. Celui du service de terminaux est binaire et encode a la main parce qu'il porte
//! des octets bruts a haute cadence ; ici on porte des appels d'interface, ou la lisibilite
//! d'un journal vaut mieux qu'un gain qui ne se mesurerait pas.
//!
//! Ce module ne connait AUCUNE fenetre : il lit son entree, repond, et pousse les
//! evenements que le backend emet. Ce qui l'affiche ne le regarde pas.

use std::io::{BufRead, Write};
use std::sync::Mutex;

use crate::evenements::Emetteur;

/// Un appel venu de l'hote. `id` revient tel quel dans la reponse : c'est ce qui permet a
/// l'hote d'avoir plusieurs appels en vol sans les confondre.
#[derive(serde::Deserialize)]
struct Appel {
    id: u64,
    commande: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

/// L'emetteur du pont : une ligne sur la sortie standard.
///
/// **LA SORTIE STANDARD EST PARTAGEE ENTRE LES FILS, D'OU LE VERROU.** Deux lignes ecrites
/// en meme temps s'entrelaceraient et l'hote lirait du JSON invalide : la sortie d'un
/// terminal arrive sur le fil du service, les reponses sur celui de la boucle.
struct EmetteurTuyau {
    sortie: Mutex<std::io::Stdout>,
}

impl EmetteurTuyau {
    fn ecrire(&self, ligne: &serde_json::Value) {
        let mut sortie = self.sortie.lock().unwrap_or_else(|e| e.into_inner());
        // Une panne d'ecriture veut dire que l'hote est parti : il n'y a rien a tenter, et
        // la boucle principale le constatera sur son entree.
        let _ = writeln!(sortie, "{ligne}");
        let _ = sortie.flush();
    }
}

impl Emetteur for EmetteurTuyau {
    fn emettre(&self, evenement: &str, charge: serde_json::Value) {
        self.ecrire(&serde_json::json!({ "evenement": evenement, "charge": charge }));
    }
}

/// Repond a un appel. Une commande inconnue est REFUSEE ET NOMMEE : rendre `null`
/// donnerait une interface qui s'affiche avec des listes vides et rien dans le journal,
/// ce qui est plus difficile a diagnostiquer qu'une panne franche.
async fn repondre(
    etat: &crate::AppState,
    commande: &str,
    _arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let valeur = |v: Result<serde_json::Value, serde_json::Error>| v.map_err(|e| e.to_string());
    match commande {
        "langue_imposee" => valeur(serde_json::to_value(crate::langue_imposee_reelle())),
        // **CHAQUE BRANCHE APPELLE LA FONCTION DE LA COMMANDE, JAMAIS SA LOGIQUE.**
        // Reecrire `etat.db.get_pending_todos()` ici donnerait deux verites pour une meme
        // reponse, et elles divergeraient au premier correctif applique d'un seul cote.
        "get_pending_todos" => {
            valeur(serde_json::to_value(crate::get_pending_todos_pour_hote(etat)?))
        }
        "get_system_metrics" => {
            valeur(serde_json::to_value(crate::get_system_metrics_pour_hote(etat).await?))
        }
        "list_projects" => {
            valeur(serde_json::to_value(crate::list_projects_pour_hote(etat).await?))
        }
        "get_app_settings" => {
            valeur(serde_json::to_value(crate::get_app_settings_pour_hote(etat)?))
        }
        "get_wallpaper" => {
            let dossier = crate::chemins::dossier_donnees()
                .ok_or("dossier de donnees inconnu")?
                .clone();
            valeur(serde_json::to_value(crate::appearance::get_wallpaper(&dossier)?))
        }
        _ => Err(format!("commande inconnue du pont : {commande}")),
    }
}

/// Sert les appels jusqu'a ce que l'hote ferme son entree.
///
/// **LA FIN DE L'ENTREE EST LA FIN DU PONT.** Un hote qui disparait laisserait sinon un
/// processus qui tient la base et le service de terminaux, invisible et sans personne pour
/// l'arreter — exactement le genre de dormeur que ce projet traque deja dans ses essais.
pub async fn servir() -> Result<(), String> {
    let dossier = crate::chemins::dossier_donnees_sans_tauri()
        .ok_or("dossier de donnees introuvable")?;
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    crate::chemins::memoriser_dossier_donnees(dossier.clone());

    let chemin_base = crate::chemin_de_la_base(&dossier);
    let db = crate::storage::Database::new(&chemin_base).map_err(|e| e.to_string())?;

    // Le tuyau est garde SOUS SON VRAI TYPE en plus d'etre vu comme un emetteur : les
    // reponses aux appels ne sont pas des evenements et n'ont pas a passer par le trait.
    let tuyau = std::sync::Arc::new(EmetteurTuyau { sortie: Mutex::new(std::io::stdout()) });
    let emetteur: crate::evenements::Emetteurs = tuyau.clone();

    let terminaux = crate::terminal::terminaux();
    terminaux.preparer(emetteur.clone(), &db);

    let etat = crate::construire_etat(db, chemin_base, terminaux);

    let entree = std::io::stdin();
    for ligne in entree.lock().lines() {
        let ligne = ligne.map_err(|e| e.to_string())?;
        if ligne.trim().is_empty() {
            continue;
        }
        let appel: Appel = match serde_json::from_str(&ligne) {
            Ok(appel) => appel,
            // Une ligne illisible ne tue pas le pont : l'hote peut avoir envoye du bruit,
            // et perdre tous les terminaux pour ca serait disproportionne.
            Err(e) => {
                emetteur.emettre("pont_erreur", serde_json::json!(e.to_string()));
                continue;
            }
        };
        let reponse = match repondre(&etat, &appel.commande, &appel.arguments).await {
            Ok(valeur) => serde_json::json!({ "id": appel.id, "ok": valeur }),
            Err(e) => serde_json::json!({ "id": appel.id, "err": e }),
        };
        tuyau.ecrire(&reponse);
    }
    Ok(())
}
