//! L'onglet Kubernetes : lire un cluster sans rien installer sur la machine.
//!
//! **CE QUE LE CLIENT ENVOIE : DES NOMS, JAMAIS UNE ADRESSE NI UN JETON.** L'interface demande
//! « le contexte `prod`, le namespace `machin` » ; c'est ce module qui va chercher dans le
//! kubeconfig ou ca pointe et avec quoi s'annoncer. Accepter une adresse venue de l'interface
//! reviendrait a lui laisser envoyer les jetons du cluster ou elle veut. Et chaque nom est
//! VERIFIE avant d'entrer dans une URL (`kubeconfig::nom_valide`).

pub mod ajout;
pub mod client;
pub mod kubeconfig;
pub mod logs;
pub mod modele;
pub mod suivi;
pub mod surveillance;
pub mod workloads;

use client::Client;
use kubeconfig::{Contexte, Fichier};
use modele::Pod;
use serde_json::Value;

/// Ce que l'ecran affiche d'un namespace, en une fois.
#[derive(Debug, serde::Serialize)]
pub struct Vue {
    pub pods: Vec<Pod>,
    /// Pourquoi il n'y a pas de CPU ni de RAM, quand c'est le cas. Une jauge vide se lirait
    /// « ce pod ne consomme rien », donc on dit pourquoi plutot que de laisser deviner.
    pub sans_mesures: Option<String>,
    /// La version de la liste, d'ou repart le flux des changements.
    pub version: String,
}

/// Les fichiers kubeconfig, lus sur le disque.
///
/// Un fichier illisible n'arrete pas les autres : `KUBECONFIG` en enchaine plusieurs, et un
/// chemin mort dedans ne doit pas priver de tous les clusters.
pub fn fichiers() -> Result<Vec<Fichier>, String> {
    let maison = crate::chemins::dossier_personnel()?;
    let variable = std::env::var("KUBECONFIG").ok();
    let chemins = kubeconfig::chemins(variable.as_deref(), &maison);
    let mut lus = Vec::new();
    let mut plaintes = Vec::new();
    for chemin in &chemins {
        match std::fs::read_to_string(chemin) {
            Ok(texte) => match kubeconfig::lire(&texte) {
                Ok(f) => lus.push(f),
                Err(e) => plaintes.push(format!("{} : {e}", chemin.display())),
            },
            // Un chemin absent est le cas ORDINAIRE quand on n'a jamais configure kubectl.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => plaintes.push(format!("{} : {e}", chemin.display())),
        }
    }
    if lus.is_empty() && !plaintes.is_empty() {
        return Err(plaintes.join(" ; "));
    }
    Ok(lus)
}

pub fn contextes() -> Result<Vec<Contexte>, String> {
    Ok(kubeconfig::contextes(&fichiers()?))
}

/// Construit le client d'un contexte NOMME.
pub fn client_de(contexte: &str) -> Result<(Client, Option<String>), String> {
    let acces = kubeconfig::acces(&fichiers()?, contexte, &|chemin| {
        std::fs::read(chemin).map_err(|e| format!("fichier du kubeconfig illisible ({chemin}) : {e}"))
    })?;
    let namespace = acces.namespace.clone();
    Ok((Client::depuis(&acces)?, namespace))
}

/// Les namespaces qu'on peut proposer dans le selecteur.
///
/// **DEUX SOURCES, ET AUCUNE N'EST GARANTIE.** Sur un cluster Rancher, la liste des namespaces
/// rend un seul nom, ou rien du tout : le droit de lister a l'echelle du cluster n'est pas
/// donne. La revue des droits en rendait 67 le matin et 403 l'apres-midi, les roles ayant
/// change entre-temps. On fusionne donc ce qu'on obtient, et **ne pas pouvoir enumerer n'est
/// PAS une erreur** : on garde l'acces aux pods du namespace qu'on vise, que l'interface laisse
/// saisir a la main. Refuser d'ouvrir l'ecran parce qu'on ne sait pas dresser la liste serait
/// interdire ce qui marche au nom de ce qui manque.
pub async fn namespaces(contexte: &str) -> Result<Vec<String>, String> {
    let (client, du_contexte) = client_de(contexte)?;
    let mut noms: Vec<String> = du_contexte.into_iter().collect();

    if let Ok(liste) = client.json("/api/v1/namespaces?limit=500").await {
        for n in liste.pointer("/items").and_then(Value::as_array).into_iter().flatten() {
            if let Some(nom) = n.pointer("/metadata/name").and_then(Value::as_str) {
                noms.push(nom.to_string());
            }
        }
    }

    // La revue des droits se demande DANS un namespace : celui qu'on vise, jamais `default`,
    // ou l'on n'a le plus souvent aucun droit — et la demande y est alors refusee.
    let ou = noms.first().cloned().unwrap_or_else(|| "default".to_string());
    let regles = client
        .poster(
            "/apis/authorization.k8s.io/v1/selfsubjectrulesreviews",
            serde_json::json!({ "spec": { "namespace": ou } }),
        )
        .await;
    if let Ok(regles) = regles {
        for regle in regles
            .pointer("/status/resourceRules")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let porte_les_namespaces = regle
                .get("resources")
                .and_then(Value::as_array)
                .is_some_and(|r| r.iter().any(|v| v.as_str() == Some("namespaces")));
            if !porte_les_namespaces {
                continue;
            }
            for nom in regle.get("resourceNames").and_then(Value::as_array).into_iter().flatten() {
                if let Some(nom) = nom.as_str() {
                    noms.push(nom.to_string());
                }
            }
        }
    }

    noms.sort();
    noms.dedup();
    Ok(noms)
}

/// La liste des pods d'un namespace, reduite, avec le CPU et la RAM quand ils sont lisibles.
pub async fn vue(contexte: &str, namespace: &str) -> Result<Vue, String> {
    if !kubeconfig::nom_valide(namespace) {
        return Err(format!("nom de namespace refuse : {namespace}"));
    }
    let (client, _) = client_de(contexte)?;
    let liste = client.json(&format!("/api/v1/namespaces/{namespace}/pods")).await?;
    let version = liste
        .pointer("/metadata/resourceVersion")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let mut pods: Vec<Pod> = liste
        .pointer("/items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(modele::reduire)
        .collect();

    // **LES MESURES SONT UN BONUS, PAS UNE CONDITION.** Un cluster sans serveur de mesures, ou
    // une identite qui n'y a pas droit, doit rendre la liste quand meme : c'est l'essentiel.
    let sans_mesures = match client
        .json(&format!("/apis/metrics.k8s.io/v1beta1/namespaces/{namespace}/pods"))
        .await
    {
        Ok(mesures) => {
            modele::poser_les_mesures(&mut pods, &mesures);
            None
        }
        Err(raison) => Some(raison),
    };

    Ok(Vue { pods, sans_mesures, version })
}

/// Ajoute un cluster au kubeconfig de la machine.
///
/// **ON ECRIT DANS LE FICHIER STANDARD, ET ON GARDE UNE COPIE DE L'ANCIEN.** C'est le fichier
/// dont depend l'acces a des productions : une ecriture ratee ou une fusion malheureuse doit
/// pouvoir se defaire. La copie porte l'heure, elle ne remplace pas la precedente.
///
/// L'ecriture passe par un fichier temporaire puis un renommage, comme pour les consignes des
/// agents : `kubectl` peut lire ce fichier a tout moment et ne doit jamais en voir une moitie.
pub async fn ajouter_un_cluster(kubeconfig: String) -> Result<ajout::Ajout, String> {
    let maison = crate::chemins::dossier_personnel()?;
    let variable = std::env::var("KUBECONFIG").ok();
    let cible = kubeconfig::chemins(variable.as_deref(), &maison)
        .into_iter()
        .next()
        .ok_or("aucun chemin de kubeconfig")?;
    ajouter_dans(&cible, &kubeconfig)
}

/// L'ecriture elle-meme, avec le chemin en parametre : c'est ce qui la rend eprouvable
/// ailleurs que dans le dossier personnel de qui fait tourner les essais.
pub fn ajouter_dans(cible: &std::path::Path, kubeconfig: &str) -> Result<ajout::Ajout, String> {
    let existant = match std::fs::read_to_string(cible) {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("kubeconfig illisible : {e}")),
    };
    let (fusionne, quoi) = ajout::fusionner(existant.as_deref(), kubeconfig)?;

    if let Some(dossier) = cible.parent() {
        std::fs::create_dir_all(dossier).map_err(|e| format!("dossier impossible a creer : {e}"))?;
        restreindre(dossier, 0o700)?;
    }
    if existant.is_some() {
        let horodatage = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let copie = cible.with_extension(format!("cockpit-{horodatage}.bak"));
        std::fs::copy(cible, &copie).map_err(|e| format!("copie de secours impossible : {e}"))?;
        restreindre(&copie, 0o600)?;
    }

    let temporaire = cible.with_extension("cockpit.tmp");
    std::fs::write(&temporaire, fusionne.as_bytes())
        .map_err(|e| format!("ecriture impossible : {e}"))?;
    restreindre(&temporaire, 0o600)?;
    std::fs::rename(&temporaire, cible).map_err(|e| format!("remplacement impossible : {e}"))?;
    Ok(quoi)
}

/// Les droits d'un fichier qui contient des jetons. Sans effet sous Windows, ou les droits
/// se decrivent autrement.
fn restreindre(chemin: &std::path::Path, mode: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(chemin, std::fs::Permissions::from_mode(mode))
            .map_err(|e| format!("droits impossibles a poser sur {} : {e}", chemin.display()))?;
    }
    #[cfg(not(unix))]
    let _ = (chemin, mode);
    Ok(())
}

/// Ce qui est DECLARE dans le namespace : services, travaux planifies, et le reste.
///
/// **UNE SORTE REFUSEE N'ARRETE PAS LES AUTRES.** Les droits se donnent ressource par
/// ressource : ne pas voir les `statefulsets` ne doit pas priver de la liste des deploiements.
pub async fn workloads(contexte: &str, namespace: &str) -> Result<Vec<workloads::Workload>, String> {
    if !kubeconfig::nom_valide(namespace) {
        return Err(format!("nom de namespace refuse : {namespace}"));
    }
    let (client, _) = client_de(contexte)?;
    let mut tout = Vec::new();
    for (groupe, ressource) in workloads::SOURCES {
        let chemin = format!("/{groupe}/namespaces/{namespace}/{ressource}");
        let Ok(liste) = client.json(&chemin).await else { continue };
        let sorte = workloads::sorte_de(ressource);
        for objet in liste.pointer("/items").and_then(Value::as_array).into_iter().flatten() {
            tout.push(workloads::reduire(objet, sorte));
        }
    }
    Ok(tout)
}

/// Supprime un pod.
///
/// **LE CLUSTER TRANCHE, PAS NOUS.** On ne regarde aucun droit de notre cote : on demande, et
/// un refus (403) revient tel quel a l'ecran. Cacher le bouton ne protegerait rien, et le
/// montrer sans pouvoir l'expliquer ferait chercher une panne chez nous.
///
/// Ce que ca fait vraiment : un pod tenu par un controleur (deploiement, travail) est RECREE
/// par lui. C'est le geste courant pour repartir sur un pod casse, et c'est ce que fait
/// l'interface web du cluster. Le travail lui-meme, ou le deploiement, n'est pas touche.
pub async fn supprimer_un_pod(contexte: &str, namespace: &str, pod: &str) -> Result<(), String> {
    for nom in [namespace, pod] {
        if !kubeconfig::nom_valide(nom) {
            return Err(format!("nom refuse : {nom}"));
        }
    }
    let (client, _) = client_de(contexte)?;
    client
        .supprimer(&format!("/api/v1/namespaces/{namespace}/pods/{pod}"))
        .await
        .map(|_| ())
}

/// Lance une tache planifiee maintenant, sans attendre sa planification. Rend le nom du
/// travail cree.
///
/// Meme regle que la suppression : des NOMS valides avant d'entrer dans une URL, et le
/// cluster tranche les droits (un refus remonte tel quel). On relit la tache au moment du
/// geste, jamais une copie gardee par l'interface : le modele a pu changer depuis l'ouverture.
pub async fn lancer_une_tache(contexte: &str, namespace: &str, tache: &str) -> Result<String, String> {
    for nom in [namespace, tache] {
        if !kubeconfig::nom_valide(nom) {
            return Err(format!("nom refuse : {nom}"));
        }
    }
    let (client, _) = client_de(contexte)?;
    let lue = client.json(&format!("/apis/batch/v1/namespaces/{namespace}/cronjobs/{tache}")).await?;
    let secondes = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let travail = workloads::travail_depuis_tache(&lue, secondes)?;
    let cree = client.poster(&format!("/apis/batch/v1/namespaces/{namespace}/jobs"), travail).await?;
    Ok(cree.pointer("/metadata/name").and_then(Value::as_str).unwrap_or_default().to_string())
}

/// Les logs d'un conteneur. `lignes` borne ce qu'on demande : un pod bavard a des centaines de
/// milliers de lignes, et les demander toutes bloquerait l'ecran le temps de les avaler.
pub async fn logs(
    contexte: &str,
    namespace: &str,
    pod: &str,
    conteneur: Option<&str>,
    lignes: u32,
    precedent: bool,
) -> Result<String, String> {
    for nom in [namespace, pod] {
        if !kubeconfig::nom_valide(nom) {
            return Err(format!("nom refuse : {nom}"));
        }
    }
    let (client, _) = client_de(contexte)?;
    let mut chemin = format!(
        "/api/v1/namespaces/{namespace}/pods/{pod}/log?tailLines={}&timestamps=true",
        lignes.clamp(1, 10_000)
    );
    if let Some(c) = conteneur.filter(|c| kubeconfig::nom_valide(c)) {
        chemin.push_str(&format!("&container={c}"));
    }
    if precedent {
        // Les logs de l'instance d'AVANT : le seul endroit ou lire pourquoi il a redemarre.
        chemin.push_str("&previous=true");
    }
    // **PAS D'`Accept: text/plain` ICI.** Le point d'entree des logs rend bien du texte, mais
    // il REFUSE cet en-tete : « 406 Not Acceptable : only application/json, application/yaml,
    // application/vnd.kubernetes.protobuf ». Vu a l'ecran, pas a la relecture — l'essai monte
    // en Python n'envoyait aucun Accept et passait donc sans rien prouver.
    client.texte(&chemin, "*/*").await
}

/// Les evenements qui concernent un pod, du plus recent au plus ancien.
pub async fn evenements(contexte: &str, namespace: &str, pod: &str) -> Result<Vec<Value>, String> {
    for nom in [namespace, pod] {
        if !kubeconfig::nom_valide(nom) {
            return Err(format!("nom refuse : {nom}"));
        }
    }
    let (client, _) = client_de(contexte)?;
    let chemin = format!(
        "/api/v1/namespaces/{namespace}/events?fieldSelector=involvedObject.name={pod}&limit=50"
    );
    let liste = client.json(&chemin).await?;
    let mut evenements: Vec<Value> = liste
        .pointer("/items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    evenements.sort_by_key(|e| {
        std::cmp::Reverse(
            e.pointer("/lastTimestamp")
                .or_else(|| e.pointer("/eventTime"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        )
    });
    Ok(evenements)
}

/// Le YAML d'un pod, tel que le cluster le rend.
pub async fn yaml(contexte: &str, namespace: &str, pod: &str) -> Result<String, String> {
    for nom in [namespace, pod] {
        if !kubeconfig::nom_valide(nom) {
            return Err(format!("nom refuse : {nom}"));
        }
    }
    let (client, _) = client_de(contexte)?;
    client
        .texte(&format!("/api/v1/namespaces/{namespace}/pods/{pod}"), "application/yaml")
        .await
}

#[cfg(test)]
mod essais_reels {
    //! Ces essais parlent a un VRAI cluster : ils sont `ignore` par defaut, et ne tournent que
    //! sur demande (`cargo test -- --ignored k8s`). Rien du cluster n'est ecrit ici : le
    //! contexte et le namespace viennent de l'environnement, et l'essai se saute proprement
    //! quand la machine n'a pas de kubeconfig.

    fn ou_essayer() -> Option<(String, String)> {
        let contexte = super::contextes().ok()?.into_iter().find(|c| c.courant)?;
        let namespace = std::env::var("COCKPIT_ESSAI_NAMESPACE")
            .ok()
            .or_else(|| contexte.namespace.clone())?;
        Some((contexte.nom, namespace))
    }

    #[tokio::test]
    #[ignore = "demande un cluster joignable"]
    async fn on_lit_un_vrai_namespace() {
        let Some((contexte, namespace)) = ou_essayer() else {
            eprintln!("aucun contexte courant utilisable : essai saute");
            return;
        };
        let debut = std::time::Instant::now();
        let vue = super::vue(&contexte, &namespace).await.expect("la vue doit se lire");
        let duree = debut.elapsed();
        let avec_mesures = vue.pods.iter().filter(|p| p.cpu.is_some()).count();
        let groupes: std::collections::BTreeSet<_> =
            vue.pods.iter().map(|p| p.groupe.clone()).collect();
        eprintln!(
            "{} pods, {} groupes, {} avec mesures, en {} ms (mesures : {})",
            vue.pods.len(),
            groupes.len(),
            avec_mesures,
            duree.as_millis(),
            vue.sans_mesures.as_deref().unwrap_or("disponibles"),
        );
        assert!(!vue.pods.is_empty(), "un namespace vide ne prouve rien");
        assert!(!vue.version.is_empty(), "sans version, le flux ne peut pas repartir de la");
        assert!(
            vue.pods.iter().all(|p| !p.nom.is_empty() && !p.groupe.is_empty()),
            "chaque ligne doit etre affichable",
        );
    }

    #[tokio::test]
    #[ignore = "demande un cluster joignable"]
    async fn les_logs_d_un_vrai_pod_se_lisent() {
        // L'essai qui manquait : le 406 des logs n'etait visible que depuis l'application.
        let Some((contexte, namespace)) = ou_essayer() else { return };
        let vue = super::vue(&contexte, &namespace).await.expect("la vue");
        let Some(pod) = vue.pods.iter().find(|p| p.etat == "Running") else {
            eprintln!("aucun pod en marche : essai saute");
            return;
        };
        let texte = super::logs(&contexte, &namespace, &pod.nom, None, 5, false)
            .await
            .expect("les logs doivent se lire");
        eprintln!("logs de {} : {} octets", pod.nom, texte.len());
    }

    #[tokio::test]
    #[ignore = "demande un cluster joignable"]
    async fn le_yaml_et_les_evenements_d_un_vrai_pod_se_lisent() {
        let Some((contexte, namespace)) = ou_essayer() else { return };
        let vue = super::vue(&contexte, &namespace).await.expect("la vue");
        let Some(pod) = vue.pods.first() else { return };
        let yaml = super::yaml(&contexte, &namespace, &pod.nom).await.expect("le yaml");
        assert!(yaml.contains("apiVersion"), "ce n'est pas du yaml : {}", &yaml[..60.min(yaml.len())]);
        let evenements = super::evenements(&contexte, &namespace, &pod.nom).await.expect("les evenements");
        eprintln!("yaml : {} octets, evenements : {}", yaml.len(), evenements.len());
    }

    #[tokio::test]
    #[ignore = "demande un cluster joignable"]
    async fn le_selecteur_de_namespaces_se_remplit() {
        let Some((contexte, _)) = ou_essayer() else { return };
        let debut = std::time::Instant::now();
        let noms = super::namespaces(&contexte).await.expect("des namespaces");
        eprintln!("{} namespaces en {} ms", noms.len(), debut.elapsed().as_millis());
        assert!(!noms.is_empty());
    }
}

#[cfg(test)]
mod tests_suppression {
    //! Le seul appel de ce module qui DETRUIT quelque chose.

    /// **UN NOM VENU DE L'INTERFACE NE RENTRE JAMAIS TEL QUEL DANS UNE URL.** Sans ce refus,
    /// `../../autre-namespace/pods/x` sortirait du namespace demande — et cet appel-la
    /// supprime. La verification a lieu AVANT tout acces au reseau : l'essai le prouve en
    /// visant un contexte qui n'existe pas, dont l'erreur serait differente.
    #[tokio::test]
    async fn un_nom_qui_sort_du_namespace_est_refuse() {
        for (ns, pod) in [
            ("../secrets", "web"),
            ("equipe", "../../autre/pods/web"),
            ("equipe", "MAJUSCULES"),
            ("equipe", ""),
        ] {
            let erreur = super::supprimer_un_pod("contexte-inexistant", ns, pod)
                .await
                .expect_err("un nom refuse doit l'etre");
            assert!(
                erreur.starts_with("nom refuse"),
                "« {ns} / {pod} » aurait du etre refuse sur son NOM, pas plus loin : {erreur}"
            );
        }
    }
}

#[cfg(test)]
mod tests_ecriture {
    //! L'ecriture du kubeconfig : ce qui protege le fichier dont depend l'acces aux clusters.

    fn dossier() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cockpit-k8s-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const QLF: &str = r#"
clusters: [{ name: qlf, cluster: { server: "https://qlf.exemple.test" } }]
contexts: [{ name: qlf, context: { cluster: qlf, user: qlf } }]
users: [{ name: qlf, user: { token: jeton-de-qlf } }]
"#;

    #[test]
    fn le_premier_ajout_cree_le_fichier_avec_des_droits_restreints() {
        let cible = dossier().join(".kube").join("config");
        std::fs::create_dir_all(cible.parent().unwrap()).unwrap();
        let quoi = super::ajouter_dans(&cible, QLF).unwrap();
        assert_eq!(quoi.ajoutes, vec!["qlf"]);
        let texte = std::fs::read_to_string(&cible).unwrap();
        assert!(texte.contains("jeton-de-qlf"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&cible).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "un fichier qui porte des jetons ne se lit pas a plusieurs");
        }
    }

    #[test]
    fn un_ajout_laisse_une_copie_de_l_ancien_fichier() {
        // **C'EST LE FILET.** Une fusion malheureuse sur ce fichier coute l'acces a des
        // clusters de production : il faut pouvoir revenir en arriere sans rien redemander.
        let d = dossier();
        let cible = d.join("config");
        let avant = "clusters: [{ name: prod, cluster: { server: \"https://prod.exemple.test\" } }]\ncontexts: [{ name: prod, context: { cluster: prod, user: prod } }]\nusers: [{ name: prod, user: { token: jeton-de-prod } }]\n";
        std::fs::write(&cible, avant).unwrap();
        super::ajouter_dans(&cible, QLF).unwrap();

        let copies: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("cockpit-"))
            .collect();
        assert_eq!(copies.len(), 1, "une copie, et une seule");
        let copie = std::fs::read_to_string(copies[0].path()).unwrap();
        assert_eq!(copie, avant, "la copie est l'ancien fichier, a l'octet pres");
        let apres = std::fs::read_to_string(&cible).unwrap();
        assert!(apres.contains("jeton-de-prod") && apres.contains("jeton-de-qlf"));
    }

    #[test]
    fn un_contenu_qui_n_est_pas_un_kubeconfig_ne_touche_pas_au_fichier() {
        let cible = dossier().join("config");
        std::fs::write(&cible, "clusters: []\ncontexts: []\nusers: []\n").unwrap();
        let avant = std::fs::read_to_string(&cible).unwrap();
        assert!(super::ajouter_dans(&cible, "bonjour").is_err());
        assert_eq!(std::fs::read_to_string(&cible).unwrap(), avant, "rien n'a bouge");
        assert!(!cible.with_extension("cockpit.tmp").exists(), "pas de temporaire oublie");
    }
}
