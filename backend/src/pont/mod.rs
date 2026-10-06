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

mod commandes;

/// Le journal du pont, sur la SORTIE D'ERREUR.
///
/// **SANS LUI, TOUTE INSTRUMENTATION DU BACKEND PARLE DANS LE VIDE.** Le journal est pose
/// par Tauri cote application ; en mode pont, personne ne l'installait, et les `log::warn!`
/// des modules disparaissaient en silence. Constate le 2026-09-10 en cherchant pourquoi la
/// reprise des preferences rendait une liste vide : elle journalisait sa cause, et pas une
/// ligne ne sortait.
///
/// La sortie standard EST le tuyau : le journal ne peut donc aller que sur stderr, que
/// l'hote recupere et affiche.
struct JournalSurErreur;

impl log::Log for JournalSurErreur {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, ligne: &log::Record) {
        eprintln!("[{}] {}", ligne.level(), ligne.args());
    }
    fn flush(&self) {}
}

/// Un appel venu de l'hote. `id` revient tel quel dans la reponse : c'est ce qui permet a
/// l'hote d'avoir plusieurs appels en vol sans les confondre.
#[derive(serde::Deserialize)]
pub(crate) struct Appel {
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
    // Les commandes de l'application d'abord, servies par la table generee : elle appelle
    // les MEMES fonctions que les commandes Tauri, sans jamais reecrire leur logique.
    if let Some(reponse) = commandes::appeler(etat, commande, _arguments).await {
        return reponse;
    }
    let valeur = |v: Result<serde_json::Value, serde_json::Error>| v.map_err(|e| e.to_string());
    match commande {
        "langue_imposee" => valeur(serde_json::to_value(crate::langue_imposee_reelle())),
        // Les preferences d'interface laissees par la version WebKitGTK. Servie par le
        // pont et non par le catalogue genere : elle n'a pas de commande Tauri en face,
        // c'est un geste propre a la coquille.
        // **CHAQUE BRANCHE APPELLE LA FONCTION DE LA COMMANDE, JAMAIS SA LOGIQUE.**
        // Reecrire `etat.db.get_pending_todos()` ici donnerait deux verites pour une meme
        // reponse, et elles divergeraient au premier correctif applique d'un seul cote.
        "get_pending_todos" => {
            valeur(serde_json::to_value(crate::get_pending_todos(etat)?))
        }
        "get_system_metrics" => {
            valeur(serde_json::to_value(crate::get_system_metrics(etat).await?))
        }
        "list_projects" => {
            valeur(serde_json::to_value(crate::list_projects(etat).await?))
        }
        "get_app_settings" => {
            valeur(serde_json::to_value(crate::get_app_settings(etat)?))
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
    // Le journal AVANT tout le reste : ce qui echoue au demarrage doit pouvoir le dire.
    // Un logger STATIQUE : `set_boxed_logger` demande la feature `std` de la crate `log`,
    // que ce projet n'active pas. Celui-ci ne coute aucune allocation.
    static JOURNAL: JournalSurErreur = JournalSurErreur;
    let _ = log::set_logger(&JOURNAL);
    log::set_max_level(log::LevelFilter::Info);

    // Un profil invalide doit etre DIT, pas se traduire par « dossier introuvable ».
    crate::chemins::profil()?;

    let dossier = crate::chemins::calculer_le_dossier_de_donnees()
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

    let etat = crate::construire_etat(db, chemin_base, terminaux, emetteur.clone());

    // **CE QUE LE DEMARRAGE DE TAURI FAISAIT ET QUE PERSONNE NE FAISAIT PLUS.** Ces deux
    // taches vivaient dans le `setup` de Tauri : en 0.59.0 elles sont parties avec lui, sans
    // qu'une seule erreur ne le dise. La surveillance des conteneurs ne tournait donc plus —
    // l'etat d'une pile ne changeait a l'ecran qu'en rouvrant l'onglet.
    let orchestrateur = etat.orchestrator.clone();
    let pour_evenements = emetteur.clone();
    crate::taches::lancer(async move {
        crate::docker::monitor::start_status_monitor(orchestrateur, 5, move || {
            pour_evenements.emettre("status_update", serde_json::Value::Null);
        })
        .await;
    });
    // Le PATH du shell de connexion, demande UNE fois et en tache de fond : le premier
    // lancement de docker le paierait sinon sur le fil de l'appel.
    crate::commande::precharger_les_chemins();

    let etat = std::sync::Arc::new(etat);
    let pour_traiter = etat.clone();
    let sortie = tuyau.clone();
    let signaler = emetteur.clone();
    servir_les_appels(
        std::io::stdin().lock(),
        move |appel| {
            let etat = pour_traiter.clone();
            async move {
                match repondre(&etat, &appel.commande, &appel.arguments).await {
                    Ok(valeur) => serde_json::json!({ "id": appel.id, "ok": valeur }),
                    Err(e) => serde_json::json!({ "id": appel.id, "err": e }),
                }
            }
        },
        move |ligne| sortie.ecrire(&ligne),
        move |e| signaler.emettre("pont_erreur", serde_json::json!(e)),
    )
    .await;
    Ok(())
}

/// La boucle des appels : lit, LANCE, et n'attend pas.
///
/// **UN APPEL LENT NE DOIT PAS EN BLOQUER UN AUTRE.** La boucle attendait la fin de chaque
/// commande avant de lire la suivante. Une seule operation lente — se brancher sur un terminal
/// a gros historique, un `git` sur un depot enorme — tenait donc tout le reste, y compris
/// l'ouverture d'un terminal et la FRAPPE, qui passe par le meme tuyau. Signale le 2026-09-21 :
/// quarante secondes avant de voir son terminal, au lancement, avec sept sessions d'agent que
/// l'application ecoutait en fond.
///
/// Chaque reponse porte son identifiant, donc l'ordre d'arrivee n'a aucune importance pour
/// l'hote. L'ordre des FRAPPES, lui, est garanti ailleurs : l'interface n'envoie l'ecriture
/// suivante d'un terminal qu'une fois la precedente acquittee.
pub(crate) async fn servir_les_appels<R, F, Fut>(
    entree: R,
    traiter: F,
    ecrire: impl Fn(serde_json::Value) + Send + Sync + Clone + 'static,
    signaler: impl Fn(String) + Send + Sync + 'static,
) where
    R: BufRead,
    F: Fn(Appel) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = serde_json::Value> + Send + 'static,
{
    for ligne in entree.lines() {
        let Ok(ligne) = ligne else { return };
        if ligne.trim().is_empty() {
            continue;
        }
        let appel: Appel = match serde_json::from_str(&ligne) {
            Ok(appel) => appel,
            // Une ligne illisible ne tue pas le pont : l'hote peut avoir envoye du bruit,
            // et perdre tous les terminaux pour ca serait disproportionne.
            Err(e) => {
                signaler(e.to_string());
                continue;
            }
        };
        let traiter = traiter.clone();
        let ecrire = ecrire.clone();
        crate::taches::lancer(async move {
            ecrire(traiter(appel).await);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex as MutexStd};

    /// **UN APPEL LENT NE BLOQUE PLUS LES AUTRES.** La boucle attendait la fin de chaque
    /// commande avant de lire la suivante : se brancher sur sept terminaux a gros historique
    /// au lancement tenait donc l'ouverture du premier terminal, et la frappe avec.
    ///
    /// L'essai envoie un appel lent PUIS un rapide, et attend la reponse du rapide bien avant
    /// celle du lent. Avec l'ancienne boucle, il tombe : la premiere reponse serait la lente.
    #[tokio::test]
    async fn un_appel_lent_ne_retient_pas_les_suivants() {
        let entree = std::io::Cursor::new(
            "{\"id\":1,\"commande\":\"lent\",\"arguments\":null}\n\
             {\"id\":2,\"commande\":\"rapide\",\"arguments\":null}\n",
        );
        let recues: Arc<MutexStd<Vec<u64>>> = Arc::new(MutexStd::new(Vec::new()));
        let vues = recues.clone();

        servir_les_appels(
            entree,
            |appel| async move {
                if appel.commande == "lent" {
                    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                }
                serde_json::json!({ "id": appel.id })
            },
            move |ligne| {
                let id = ligne.get("id").and_then(serde_json::Value::as_u64).unwrap_or(0);
                vues.lock().unwrap_or_else(|e| e.into_inner()).push(id);
            },
            |_| {},
        )
        .await;

        // La boucle a rendu la main tout de suite : elle ne retient plus rien.
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert_eq!(
            recues.lock().unwrap_or_else(|e| e.into_inner()).as_slice(),
            &[2],
            "le rapide doit avoir repondu pendant que le lent travaille encore"
        );

        tokio::time::sleep(std::time::Duration::from_millis(700)).await;
        assert_eq!(
            recues.lock().unwrap_or_else(|e| e.into_inner()).as_slice(),
            &[2, 1],
            "et le lent repond ensuite, sans etre perdu"
        );
    }

    /// Une ligne illisible ne tue pas le pont : elle est NOMMEE, et la suivante est servie.
    #[tokio::test]
    async fn une_ligne_illisible_ne_tue_pas_le_pont() {
        let entree = std::io::Cursor::new(
            "ceci n'est pas du JSON\n{\"id\":7,\"commande\":\"ok\",\"arguments\":null}\n",
        );
        let recues: Arc<MutexStd<Vec<u64>>> = Arc::new(MutexStd::new(Vec::new()));
        let vues = recues.clone();
        let plaintes: Arc<MutexStd<usize>> = Arc::new(MutexStd::new(0));
        let comptees = plaintes.clone();

        servir_les_appels(
            entree,
            |appel| async move { serde_json::json!({ "id": appel.id }) },
            move |ligne| {
                let id = ligne.get("id").and_then(serde_json::Value::as_u64).unwrap_or(0);
                vues.lock().unwrap_or_else(|e| e.into_inner()).push(id);
            },
            move |_| *comptees.lock().unwrap_or_else(|e| e.into_inner()) += 1,
        )
        .await;

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(*plaintes.lock().unwrap_or_else(|e| e.into_inner()), 1, "le bruit est nomme");
        assert_eq!(recues.lock().unwrap_or_else(|e| e.into_inner()).as_slice(), &[7]);
    }
}
