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
    let demande = std::env::var_os(NOTRE_REGLAGE).is_some_and(|v| v != "0");
    let mode = if demande {
        std::env::set_var(VARIABLE, "1");
        format!("rendu : DMA-BUF desactive ({NOTRE_REGLAGE} demande)")
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
}
