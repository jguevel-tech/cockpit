//! Le mode de rendu de la vue web, decide AVANT l'initialisation de GTK.
//!
//! **Le probleme, mesure le 2026-08-31.** Le journal du guetteur montre des episodes ou la page
//! rend compte normalement mais ne peint AUCUNE image : le backend va bien, la boucle graphique
//! repond, et la fenetre est morte a l'ecran. Il faut tuer l'application pour en sortir.
//!
//! C'est un defaut connu de WebKitGTK avec le pilote proprietaire NVIDIA : le chemin de rendu
//! par DMA-BUF que le moteur prefere n'y fonctionne pas de facon fiable. Le rapport amont
//! s'intitule « [GTK] Disable DMABuf renderer for NVIDIA proprietary drivers », et le
//! contournement partage par tout l'ecosysteme est une variable d'environnement posee avant que
//! le moteur ne demarre. Machine concernee ici : RTX 500 Ada avec le pilote `nvidia`, session
//! Wayland, WebKitGTK 2.52.
//!
//! **MAIS LE CONTOURNEMENT A UN PRIX, ET IL A ETE PAYE DEUX FOIS.** Sans le chemin DMA-BUF, la
//! page est composee par le processeur : interface plus lente, et surtout une frappe qui arrive
//! en retard dans les terminaux — on voit l'effet de la touche PRECEDENTE a chaque touche. La
//! 0.54.2 l'imposait des que le pilote NVIDIA etait present, la 0.54.3 est revenue en arriere
//! pour cette raison ; la 0.54.7 l'a remis AUTOMATIQUEMENT sous NVIDIA + Wayland, et la meme
//! plainte est revenue le jour meme. Un gel apres une veille se recupere en relancant ; une
//! frappe en retard rend le logiciel inutilisable a chaque minute. Le contournement n'est donc
//! JAMAIS decide a la place de l'utilisateur : `COCKPIT_SANS_DMABUF=1` le demande, rien d'autre
//! ne l'active. Le journal du guetteur dit comment le poser quand la fenetre cesse de se
//! redessiner.
//!
//! Le choix et l'ecrit dans le journal avant l'initialisation de GTK, pour rendre le diagnostic
//! verifiable sans deviner le mode actif.

/// La variable que WebKitGTK lit au demarrage. Elle n'existe que sous Linux : ce moteur n'est
/// pas celui des autres systemes.
#[cfg(target_os = "linux")]
const VARIABLE: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

/// Notre propre reglage, celui qu'on demande a l'utilisateur de poser.
///
/// **POURQUOI PAS DIRECTEMENT CELLE DE WEBKIT.** La 0.54.2 posait la variable de WebKit sur
/// elle-meme ; la mise a jour relance l'application depuis l'ancien processus, donc la 0.54.3 —
/// qui devait justement revenir en arriere — l'a HERITEE et l'a prise pour un choix de
/// l'utilisateur. Le retour en arriere etait annule sans que personne ne s'en apercoive :
/// constate le 2026-08-31 dans le journal, « pose par l'utilisateur » alors que personne ne
/// l'avait posee. Une variable a NOUS ne peut pas etre confondue avec une fuite.
#[cfg(target_os = "linux")]
const NOTRE_REGLAGE: &str = "COCKPIT_SANS_DMABUF";

/// Ce qui a ete decide, pour que le journal le dise au lieu de le laisser deviner.
static MODE: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Le fichier du mode secours : present = demande.
///
/// **POURQUOI UN FICHIER ET PAS LA BASE.** `decider()` tourne AVANT l'initialisation de
/// GTK, donc avant que l'application n'existe : ni `AppHandle`, ni base ouverte. Un fichier
/// dans le dossier de donnees se lit sans rien d'autre. Il est ecrit par l'ecran des
/// Parametres et par le dialogue que le guetteur propose apres des gels repete — donc dans
/// les deux cas sur un geste de l'utilisateur, jamais par une detection automatique : la
/// regle « le contournement n'est JAMAIS decide a la place de l'utilisateur » tient.
#[cfg(target_os = "linux")]
pub const NOM_MODE_SECOURS: &str = "mode_secours_rendu";

/// Le chemin du fichier du mode secours. `None` si le dossier personnel est introuvable.
#[cfg(target_os = "linux")]
pub fn chemin_mode_secours() -> Option<std::path::PathBuf> {
    crate::chemins::dossier_donnees_sans_tauri()
        .map(|dossier| dossier.join(NOM_MODE_SECOURS))
}

/// La demande, separee des lectures pour etre testable : la variable d'environnement OU le
/// fichier du mode secours.
#[cfg(target_os = "linux")]
fn secours_demande(valeur_reglage: Option<&std::ffi::OsStr>, drapeau_pose: bool) -> bool {
    valeur_reglage.is_some_and(|v| v != "0") || drapeau_pose
}

/// Pose ou retire le fichier du mode secours. Separe du chemin pour etre testable sur un
/// dossier temporaire : un essai qui ecrirait le VRAI fichier activerait le contournement
/// sur la machine qui execute les essais.
#[cfg(target_os = "linux")]
fn ecrire_drapeau(chemin: &std::path::Path, activer: bool) -> Result<(), String> {
    if activer {
        if let Some(parent) = chemin.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("dossier du mode secours : {e}"))?;
        }
        std::fs::write(chemin, "mode secours du rendu, active dans Cockpit\n")
            .map_err(|e| format!("ecriture du mode secours : {e}"))
    } else {
        match std::fs::remove_file(chemin) {
            Ok(()) => Ok(()),
            // Absent = deja desactive : retirer deux fois n'est pas une erreur.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("retrait du mode secours : {e}")),
        }
    }
}

/// Etat du mode secours, pour l'ecran des Parametres. `disponible` vaut `false` hors Linux :
/// le chemin DMA-BUF est celui de WebKitGTK, les autres systemes n'ont rien a en dire.
#[derive(serde::Serialize)]
pub struct EtatModeSecours {
    pub disponible: bool,
    pub actif: bool,
}

pub fn etat_mode_secours() -> EtatModeSecours {
    #[cfg(target_os = "linux")]
    {
        EtatModeSecours {
            disponible: true,
            actif: chemin_mode_secours().is_some_and(|c| c.exists()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        EtatModeSecours {
            disponible: false,
            actif: false,
        }
    }
}

/// Pose ou retire le mode secours. Hors Linux : une erreur claire, pas un silence — un
/// bouton qui ne fait rien est un mensonge.
pub fn basculer_mode_secours(activer: bool) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        let chemin = chemin_mode_secours().ok_or("dossier de donnees introuvable")?;
        ecrire_drapeau(&chemin, activer)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = activer;
        Err("le mode secours du rendu ne concerne que Linux".to_string())
    }
}

/// Le pilote proprietaire NVIDIA est-il charge ? Sert au JOURNAL, jamais a decider : c'est la
/// configuration ou le gel a ete constate, donc celle ou la ligne doit dire comment s'en sortir.
///
/// `/sys/module/nvidia` n'existe qu'avec le pilote proprietaire : ni avec nouveau, ni sur une
/// machine sans carte NVIDIA.
#[cfg(target_os = "linux")]
fn pilote_nvidia_proprietaire() -> bool {
    std::path::Path::new("/sys/module/nvidia").exists()
}

/// **A appeler AVANT toute initialisation de GTK.** WebKitGTK lit sa variable au demarrage du
/// moteur : la poser apres n'a aucun effet, et l'echec serait silencieux.
///
/// **L'ETAT EST TOUJOURS REMIS A PLAT**, dans un sens comme dans l'autre. Laisser une valeur
/// telle quelle laissait une variable HERITEE decider a notre place — c'est ainsi que le retour
/// en arriere de la 0.54.3 s'est retrouve sans effet.
#[cfg(target_os = "linux")]
pub fn decider() {
    let par_variable = std::env::var_os(NOTRE_REGLAGE);
    let drapeau_pose = chemin_mode_secours().is_some_and(|c| c.exists());
    let demande = secours_demande(par_variable.as_deref(), drapeau_pose);
    let mode = if demande {
        std::env::set_var(VARIABLE, "1");
        let origine = if drapeau_pose {
            "mode secours active dans l'application".to_string()
        } else {
            format!("{NOTRE_REGLAGE} demande")
        };
        format!("rendu : DMA-BUF desactive ({origine})")
    } else {
        std::env::remove_var(VARIABLE);
        if pilote_nvidia_proprietaire() {
            format!(
                "rendu : chemin normal, pilote NVIDIA proprietaire present — si la fenetre cesse \
                 de se redessiner, relancer avec {NOTRE_REGLAGE}=1"
            )
        } else {
            "rendu : chemin normal".to_string()
        }
    };
    let _ = MODE.set(mode);
}

#[cfg(not(target_os = "linux"))]
pub fn decider() {
    // Ce chemin de rendu n'existe que dans WebKitGTK : rien a decider ailleurs.
    let _ = MODE.set("rendu : sans objet sur ce systeme".to_string());
}

/// Ce qui a ete decide, pour le journal.
pub fn mode() -> String {
    MODE.get()
        .cloned()
        .unwrap_or_else(|| "rendu : pas encore decide".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Les deux sens de la decision, dans UN SEUL essai : ils touchent l'environnement du
    /// processus, et deux essais qui le modifient en parallele se marcheraient dessus.
    ///
    /// **LE FAUX POSITIF A NE PAS REINTRODUIRE.** Une variable de WebKit heritee d'une version
    /// precedente ne doit pas decider a notre place : la mise a jour relance l'application depuis
    /// l'ancien processus, donc tout ce qu'une version avait pose sur elle-meme se transmet.
    #[cfg(target_os = "linux")]
    #[test]
    fn l_etat_est_toujours_remis_a_plat() {
        // Ce qu'une version precedente aurait laisse derriere elle, sans que personne ne l'ait
        // demande. SANS notre reglage : rien ne doit decider a la place de l'utilisateur, meme
        // sur la machine NVIDIA + Wayland qui execute cet essai — la 0.54.7 le faisait, et la
        // frappe en retard est revenue le jour meme.
        std::env::remove_var(NOTRE_REGLAGE);
        std::env::set_var(VARIABLE, "1");
        decider();
        assert!(
            std::env::var_os(VARIABLE).is_none(),
            "la valeur heritee devait etre retiree, elle vaut {:?}",
            std::env::var_os(VARIABLE)
        );

        // La valeur 0 dit explicitement « chemin normal », et n'active rien non plus.
        std::env::set_var(NOTRE_REGLAGE, "0");
        std::env::set_var(VARIABLE, "1");
        decider();
        assert!(std::env::var_os(VARIABLE).is_none());

        // Et notre reglage, lui, est honore.
        std::env::set_var(NOTRE_REGLAGE, "1");
        decider();
        assert_eq!(std::env::var(VARIABLE).as_deref(), Ok("1"));

        // On remet les lieux en etat : d'autres essais lisent cet environnement.
        std::env::remove_var(NOTRE_REGLAGE);
        std::env::remove_var(VARIABLE);
    }

    /// La decision doit etre prise et NOMMEE : un mode inconnu dans le journal ne renseigne
    /// personne, et c'est ce journal qui jugera si le contournement a servi.
    #[test]
    fn la_decision_est_nommee() {
        decider();
        let mode = mode();
        assert!(mode.starts_with("rendu : "), "mode inattendu : {mode}");
        assert!(
            !mode.contains("pas encore decide"),
            "la decision n'a pas ete prise"
        );
    }

    /// Le fichier du mode secours est une demande au meme titre que la variable. L'essai
    /// travaille sur un dossier TEMPORAIRE : ecrire le vrai fichier activerait le
    /// contournement sur la machine qui execute les essais.
    #[cfg(target_os = "linux")]
    #[test]
    fn le_drapeau_se_pose_et_se_retire() {
        let dossier = std::env::temp_dir().join(format!("cockpit-secours-{}", std::process::id()));
        let chemin = dossier.join("sous-dossier").join(NOM_MODE_SECOURS);

        ecrire_drapeau(&chemin, true).unwrap();
        assert!(chemin.exists(), "le fichier devait etre cree, dossier compris");
        assert!(secours_demande(None, chemin.exists()));

        ecrire_drapeau(&chemin, false).unwrap();
        assert!(!chemin.exists());
        // Retirer deux fois n'est pas une erreur : l'etat vise est atteint.
        ecrire_drapeau(&chemin, false).unwrap();
        assert!(!secours_demande(None, chemin.exists()));

        // La variable seule demande aussi ; « 0 » est un refus explicite.
        assert!(secours_demande(Some(std::ffi::OsStr::new("1")), false));
        assert!(!secours_demande(Some(std::ffi::OsStr::new("0")), false));
        assert!(!secours_demande(None, false));

        let _ = std::fs::remove_dir_all(&dossier);
    }
}
