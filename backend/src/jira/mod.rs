//! Jira Server / Data Center : lister ses tickets et les faire avancer depuis Cockpit.
//!
//! **LE JETON NE QUITTE JAMAIS LE BACKEND.** L'interface recoit `jeton_pose`, pas la valeur :
//! meme regle que les cles d'API des fournisseurs d'IA.
//!
//! **TOUTES LES COMMANDES SONT `async`** : elles attendent le reseau ou git.

use marqueur_commande::commande;

pub mod branche;
pub mod client;
pub mod config;
pub mod jql;
pub mod modele;

use branche::Correspondance;
use client::Jira;
use config::{ConfigJira, LiaisonJira};
use modele::{DetailTicket, Ticket, Transition, TypeTicket};
use reqwest::Method;

const CHAMPS_LISTE: &str = "summary,status,issuetype,priority,project,updated";
const CHAMPS_DETAIL: &str = "summary,description,status,issuetype,priority,project,updated,comment";

#[commande]
pub async fn jira_config(state: &crate::AppState) -> Result<ConfigJira, String> {
    Ok(config::lire(&state.db))
}

#[commande]
pub async fn jira_poser_config(
    state: &crate::AppState,
    url: String,
    jeton: Option<String>,
    email: Option<String>,
    types_branche: Option<Correspondance>,
) -> Result<ConfigJira, String> {
    config::poser(&state.db, &url, jeton.as_deref(), email.as_deref(), types_branche)
}

/// Rend le nom affiche du compte : de quoi confirmer que c'est le bon. Cloud ne renvoie que
/// `accountId` (pas `name`) : sans lui, on ne rendrait jamais rien pour ces comptes-la.
#[commande]
pub async fn jira_tester(state: &crate::AppState) -> Result<String, String> {
    let moi = moi(&Jira::depuis(&state.db)?).await?;
    Ok(if !moi.nom_affiche.is_empty() {
        moi.nom_affiche
    } else if !moi.name.is_empty() {
        moi.name
    } else {
        moi.account_id
    })
}

/// `cles_projets` absent : tous mes tickets. Liste vide : aucun (voir `jql::mes_tickets`).
///
/// Cloud retire `/rest/api/2/search` : il faut `/rest/api/3/search/jql`, memes parametres,
/// paginee (`nextPageToken`/`isLast`) — voir `chercher_cote_cloud`.
#[commande]
pub async fn jira_mes_tickets(
    state: &crate::AppState,
    cles_projets: Option<Vec<String>>,
) -> Result<Vec<Ticket>, String> {
    let Some(jql) = jql::mes_tickets(cles_projets.as_deref())? else {
        return Ok(vec![]);
    };
    let jira = Jira::depuis(&state.db)?;
    if jira.cloud() {
        return chercher_cote_cloud(&jira, &jql).await;
    }
    let corps = jira
        .envoyer(
            Method::GET,
            "/rest/api/2/search",
            &[("jql", jql.as_str()), ("fields", CHAMPS_LISTE), ("maxResults", "200")],
            None,
        )
        .await?;
    modele::lire_recherche(&corps, jira.base())
}

/// Au-dela, on rend la main avec ce qu'on a deja : mieux vaut une liste tronquee qu'une
/// boucle qui ne rendrait jamais la main si Jira renvoyait un `nextPageToken` qui boucle.
const PAGES_MAX: u8 = 5;

/// `/rest/api/3/search/jql` (Cloud) pagine : chaque reponse porte au plus `maxResults`
/// tickets et dit s'il en reste (`isLast`/`nextPageToken`). Il faut boucler pour tout avoir.
async fn chercher_cote_cloud(jira: &Jira, jql: &str) -> Result<Vec<Ticket>, String> {
    let mut tickets = Vec::new();
    let mut jeton: Option<String> = None;
    for _ in 0..PAGES_MAX {
        let mut requete = vec![("jql", jql), ("fields", CHAMPS_LISTE), ("maxResults", "200")];
        if let Some(j) = jeton.as_deref() {
            requete.push(("nextPageToken", j));
        }
        let corps = jira.envoyer(Method::GET, "/rest/api/3/search/jql", &requete, None).await?;
        let (page, suite) = modele::lire_page_recherche(&corps, jira.base())?;
        tickets.extend(page);
        match suite {
            Some(j) => jeton = Some(j),
            None => break,
        }
    }
    Ok(tickets)
}

#[commande]
pub async fn jira_ticket(state: &crate::AppState, cle: String) -> Result<DetailTicket, String> {
    lire_le_ticket(&Jira::depuis(&state.db)?, &cle).await
}

#[commande]
pub async fn jira_transitions(state: &crate::AppState, cle: String) -> Result<Vec<Transition>, String> {
    lister_transitions(&Jira::depuis(&state.db)?, &cle).await
}

#[commande]
pub async fn jira_liaisons(state: &crate::AppState) -> Result<Vec<LiaisonJira>, String> {
    config::liaisons(&state.db)
}

#[commande]
pub async fn jira_liaison(state: &crate::AppState, projet: String) -> Result<LiaisonJira, String> {
    let nom = crate::resolve_db_project_name(state, &projet).await;
    config::liaison(&state.db, &nom)
}

#[commande]
pub async fn jira_poser_liaison(
    state: &crate::AppState,
    projet: String,
    cles: String,
    gabarit: String,
) -> Result<LiaisonJira, String> {
    let nom = crate::resolve_db_project_name(state, &projet).await;
    config::poser_liaison(&state.db, &nom, &cles, &gabarit)
}

/// L'apercu calcule ici plutot que dans l'interface : une seule regle de nommage.
#[commande]
pub async fn jira_apercu_branche(
    state: &crate::AppState,
    gabarit: String,
    cle: String,
    type_ticket: String,
    resume: String,
) -> Result<String, String> {
    Ok(branche::nom_de_branche(&gabarit, &cle, &type_ticket, &resume, &config::types_branche(&state.db)))
}

#[commande]
pub async fn jira_transitionner(state: &crate::AppState, cle: String, transition_id: String) -> Result<(), String> {
    transitionner(&Jira::depuis(&state.db)?, &cle, &transition_id).await
}

#[commande]
pub async fn jira_commenter(state: &crate::AppState, cle: String, texte: String) -> Result<(), String> {
    jql::verifier_cle_de_ticket(&cle)?;
    let texte = texte.trim();
    if texte.is_empty() {
        return Err("commentaire vide".to_string());
    }
    Jira::depuis(&state.db)?
        .envoyer(
            Method::POST,
            &format!("/rest/api/2/issue/{cle}/comment"),
            &[],
            Some(serde_json::json!({ "body": texte })),
        )
        .await
        .map(|_| ())
}

#[commande]
pub async fn jira_saisir_temps(
    state: &crate::AppState,
    cle: String,
    duree: String,
    commentaire: Option<String>,
) -> Result<(), String> {
    jql::verifier_cle_de_ticket(&cle)?;
    let corps = modele::corps_de_saisie(&duree, commentaire.as_deref())?;
    Jira::depuis(&state.db)?
        .envoyer(Method::POST, &format!("/rest/api/2/issue/{cle}/worklog"), &[], Some(corps))
        .await
        .map(|_| ())
}

#[commande]
pub async fn jira_types_ticket(state: &crate::AppState, cle_projet: String) -> Result<Vec<TypeTicket>, String> {
    let cle_projet = jql::normaliser_cle_de_projet(&cle_projet)?;
    let corps = Jira::depuis(&state.db)?
        .envoyer(Method::GET, &format!("/rest/api/2/issue/createmeta/{cle_projet}/issuetypes"), &[], None)
        .await?;
    modele::lire_types(&corps)
}

/// Cree le ticket en me l'assignant, et rend sa cle. Sur les instances Jira ou `assignee`
/// n'est pas sur l'ecran de creation, retente sans ce champ puis assigne a part (voir
/// `assigner_apres_coup`) : le ticket existe deja, mieux vaut le rendre non assigne que de
/// perdre la creation pour un champ que Jira ne veut pas au bon endroit.
#[commande]
pub async fn jira_creer_ticket(
    state: &crate::AppState,
    cle_projet: String,
    type_id: String,
    resume: String,
    description: Option<String>,
) -> Result<String, String> {
    let cle_projet = jql::normaliser_cle_de_projet(&cle_projet)?;
    let jira = Jira::depuis(&state.db)?;
    let moi = moi(&jira).await?;
    let assigne = modele::assigne_de(&moi, jira.cloud());
    let corps = modele::corps_de_creation(&cle_projet, &type_id, &resume, description.as_deref(), assigne)?;
    match jira.envoyer(Method::POST, "/rest/api/2/issue", &[], Some(corps)).await {
        Ok(reponse) => modele::lire_ticket_cree(&reponse),
        Err(e) if modele::refus_du_champ_assignee(&e) => {
            let corps = modele::corps_de_creation(&cle_projet, &type_id, &resume, description.as_deref(), None)?;
            let reponse = jira.envoyer(Method::POST, "/rest/api/2/issue", &[], Some(corps)).await?;
            let cle = modele::lire_ticket_cree(&reponse)?;
            assigner_apres_coup(&jira, &cle, assigne).await;
            Ok(cle)
        }
        Err(e) => Err(e),
    }
}

/// Le ticket existe deja : un echec ici n'annule rien, on le journalise seulement. Meme objet
/// que sur la creation (`{"accountId": ..}` sur Cloud, `{"name": ..}` sinon). `None` (compte
/// sans identifiant utilisable) : rien a envoyer, on ne tente meme pas la requete.
async fn assigner_apres_coup(jira: &Jira, cle: &str, assigne: Option<modele::Assigne<'_>>) {
    let Some(assigne) = assigne else { return };
    let resultat = jira
        .envoyer(Method::PUT, &format!("/rest/api/2/issue/{cle}/assignee"), &[], Some(assigne.valeur()))
        .await;
    if let Err(e) = resultat {
        log::warn!("jira : ticket {cle} cree mais non assigne - {e}");
    }
}

async fn transitionner(jira: &Jira, cle: &str, transition_id: &str) -> Result<(), String> {
    jql::verifier_cle_de_ticket(cle)?;
    jira.envoyer(
        Method::POST,
        &format!("/rest/api/2/issue/{cle}/transitions"),
        &[],
        Some(serde_json::json!({ "transition": { "id": transition_id } })),
    )
    .await
    .map(|_| ())
}

async fn moi(jira: &Jira) -> Result<modele::Moi, String> {
    let corps = jira.envoyer(Method::GET, "/rest/api/2/myself", &[], None).await?;
    modele::lire_moi(&corps)
}

async fn lire_le_ticket(jira: &Jira, cle: &str) -> Result<DetailTicket, String> {
    jql::verifier_cle_de_ticket(cle)?;
    let corps = jira
        .envoyer(Method::GET, &format!("/rest/api/2/issue/{cle}"), &[("fields", CHAMPS_DETAIL)], None)
        .await?;
    modele::lire_ticket(&corps, jira.base())
}

async fn lister_transitions(jira: &Jira, cle: &str) -> Result<Vec<Transition>, String> {
    jql::verifier_cle_de_ticket(cle)?;
    let corps = jira
        .envoyer(Method::GET, &format!("/rest/api/2/issue/{cle}/transitions"), &[], None)
        .await?;
    modele::lire_transitions(&corps)
}

#[derive(Debug, serde::Serialize)]
pub struct Demarrage {
    pub branche: String,
    pub creee: bool,
    pub base: Option<String>,
    /// Le nom de la transition appliquee ; `None` si le ticket etait deja en cours.
    pub transition: Option<String>,
    /// **LA BRANCHE EST GARDEE MEME SI LE STATUT N'A PAS SUIVI** : on le dit, on n'annule pas.
    pub erreur_transition: Option<String>,
}

/// Le dossier du depot et la liaison d'un projet Cockpit.
async fn depot_et_liaison(state: &crate::AppState, projet: &str) -> Result<(String, LiaisonJira), String> {
    let nom = crate::resolve_db_project_name(state, projet).await;
    let depot = state.db.get_project_by_name(&nom)?.path;
    if depot.trim().is_empty() {
        return Err(format!("le projet {projet} n'a pas de dossier sur cette machine"));
    }
    Ok((depot, config::liaison(&state.db, &nom)?))
}

fn branche_du_ticket(state: &crate::AppState, liaison: &LiaisonJira, ticket: &Ticket) -> String {
    branche::nom_de_branche(
        &liaison.gabarit,
        &ticket.cle,
        &ticket.type_ticket,
        &ticket.resume,
        &config::types_branche(&state.db),
    )
}

#[derive(Debug, serde::Serialize)]
pub struct BrancheJira {
    pub nom: String,
    /// Vide : branche sans ticket.
    pub cles: Vec<String>,
}

/// Les branches locales du projet et les tickets qu'elles citent. Pas d'appel a Jira : un
/// ticket cite mais absent de mes tickets (assigne a un autre) compte quand meme.
#[commande]
pub async fn jira_branches(state: &crate::AppState, projet: String) -> Result<Vec<BrancheJira>, String> {
    let (depot, liaison) = depot_et_liaison(state, &projet).await?;
    Ok(crate::gitdiff::branches::lister(&depot)
        .await?
        .into_iter()
        .map(|nom| BrancheJira { cles: branche::cles_dans(&nom, &liaison.cles), nom })
        .collect())
}

/// Renomme une branche d'apres le gabarit du ticket, et rend le nouveau nom. Locale
/// seulement : une branche deja poussee garde son ancien nom sur le depot distant.
#[commande]
pub async fn jira_renommer_branche(
    state: &crate::AppState,
    projet: String,
    branche: String,
    cle: String,
) -> Result<String, String> {
    let (depot, liaison) = depot_et_liaison(state, &projet).await?;
    let ticket = lire_le_ticket(&Jira::depuis(&state.db)?, &cle).await?.ticket;
    let nouvelle = branche_du_ticket(state, &liaison, &ticket);
    crate::gitdiff::branches::renommer(&depot, &branche, &nouvelle).await?;
    Ok(nouvelle)
}

#[commande]
pub async fn jira_demarrer(state: &crate::AppState, projet: String, cle: String) -> Result<Demarrage, String> {
    let jira = Jira::depuis(&state.db)?;
    let (depot, liaison) = depot_et_liaison(state, &projet).await?;
    let ticket = lire_le_ticket(&jira, &cle).await?.ticket;
    let branche = branche_du_ticket(state, &liaison, &ticket);
    let depart = crate::gitdiff::depart::partir_de_la_base(&depot, &branche).await?;
    let (transition, erreur_transition) = passer_en_cours(&jira, &ticket).await;
    Ok(Demarrage { branche, creee: depart.creee, base: depart.base, transition, erreur_transition })
}

async fn passer_en_cours(jira: &Jira, ticket: &Ticket) -> (Option<String>, Option<String>) {
    if ticket.categorie_statut == "indeterminate" {
        return (None, None);
    }
    let transitions = match lister_transitions(jira, &ticket.cle).await {
        Ok(t) => t,
        Err(e) => return (None, Some(e)),
    };
    let Some(t) = modele::transition_en_cours(&transitions) else {
        return (None, Some("aucune transition vers un statut en cours n'est proposee pour ce ticket".to_string()));
    };
    match transitionner(jira, &ticket.cle, &t.id).await {
        Ok(()) => (Some(t.nom.clone()), None),
        Err(e) => (None, Some(e)),
    }
}
