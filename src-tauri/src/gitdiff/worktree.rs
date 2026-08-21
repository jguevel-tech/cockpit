//! Les worktrees git d'un projet : lister, ajouter, retirer.
//!
//! Un worktree est un second dossier de travail sur le MEME depot, sur une autre branche. Ce
//! que ca sert ici : faire tourner plusieurs agents en parallele, chacun dans son dossier, sans
//! qu'ils se marchent dessus — un `git checkout` dans un seul dossier changerait le code sous
//! les pieds des autres.
//!
//! **Ou ils sont crees** : dans un dossier FRERE du projet, nomme
//! `<dossier-du-projet>.worktrees/<branche>`. Trois raisons : le depot lui-meme reste propre
//! (rien a ignorer, rien qui apparaisse dans l'onglet Fichiers) ; tout est regroupe au meme
//! endroit, donc supprimable d'un geste ; et le chemin est previsible, donc affichable et
//! retrouvable a la main. Le chemin complet est TOUJOURS montre a l'utilisateur : rien ne doit
//! apparaitre sur son disque sans qu'il sache ou.

use super::{run_git, run_git_strict};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Un dossier de travail du depot. Le principal en fait partie, avec `principal = true`.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Worktree {
    pub chemin: String,
    /// Nom court de la branche, ou `None` quand la tete est detachee.
    pub branche: Option<String>,
    /// Debut du hash de la tete, pour situer un worktree detache.
    pub tete: String,
    pub principal: bool,
    pub verrouille: bool,
    /// Git considere ce worktree comme supprimable (dossier disparu, par exemple).
    pub elagable: bool,
}

/// Le dossier qui regroupe les worktrees d'un projet.
///
/// Frere du projet et non enfant : un worktree DANS le depot serait vu par l'onglet Fichiers,
/// par les recherches, et par git lui-meme comme du contenu non suivi.
pub fn dossier_des_worktrees(projet: &str) -> PathBuf {
    let chemin = Path::new(projet);
    let nom = chemin.file_name().map(|n| n.to_string_lossy().to_string());
    let parent = chemin.parent().map(|p| p.to_path_buf());
    match (parent, nom) {
        (Some(parent), Some(nom)) => parent.join(format!("{nom}.worktrees")),
        // Un projet a la racine du systeme de fichiers : on retombe dedans plutot que de
        // refuser, le cas est absurde mais ne doit pas paniquer.
        _ => chemin.join(".worktrees"),
    }
}

/// Nom de dossier tire d'un nom de branche.
///
/// Une branche peut contenir des `/` (`feat/truc`), qui creeraient une hierarchie de dossiers
/// inutile, et des caracteres qu'un systeme de fichiers refuse. On ne garde que des caracteres
/// sans surprise, et on refuse ce qui pourrait sortir du dossier.
pub fn nom_de_dossier(branche: &str) -> Result<String, String> {
    let nettoye: String = branche
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '-' })
        .collect();
    let nettoye = nettoye.trim_matches(['-', '.']).to_string();
    if nettoye.is_empty() {
        return Err("nom de branche vide".into());
    }
    Ok(nettoye)
}

/// Les worktrees du depot, le principal en premier.
///
/// `git worktree list --porcelain` rend des blocs separes par une ligne vide. Le format est
/// STABLE (git le documente comme tel), au contraire de la sortie lisible : c'est pour ca qu'on
/// l'utilise plutot que de decouper des colonnes.
pub async fn lister(repo: &str) -> Result<Vec<Worktree>, String> {
    let brut = run_git(repo, &["worktree", "list", "--porcelain"]).await?;
    Ok(analyser(&brut))
}

/// La partie PURE de `lister`, pour qu'elle soit testable sans depot.
fn analyser(brut: &str) -> Vec<Worktree> {
    let mut sortie = Vec::new();
    let mut courant: Option<Worktree> = None;
    for ligne in brut.lines() {
        if let Some(chemin) = ligne.strip_prefix("worktree ") {
            if let Some(fini) = courant.take() {
                sortie.push(fini);
            }
            courant = Some(Worktree {
                chemin: chemin.to_string(),
                branche: None,
                tete: String::new(),
                // Le PREMIER bloc est le worktree principal : git le rend toujours en tete.
                principal: sortie.is_empty(),
                verrouille: false,
                elagable: false,
            });
            continue;
        }
        let Some(w) = courant.as_mut() else { continue };
        if let Some(tete) = ligne.strip_prefix("HEAD ") {
            w.tete = tete.chars().take(8).collect();
        } else if let Some(reference) = ligne.strip_prefix("branch ") {
            w.branche = Some(reference.trim_start_matches("refs/heads/").to_string());
        } else if ligne == "locked" || ligne.starts_with("locked ") {
            w.verrouille = true;
        } else if ligne == "prunable" || ligne.starts_with("prunable ") {
            w.elagable = true;
        }
    }
    if let Some(fini) = courant {
        sortie.push(fini);
    }
    sortie
}

/// Ajoute un worktree sur `branche`, en la creant si `creer`. Rend son chemin.
///
/// Git refuse une branche deja sortie dans un autre worktree, et le dit clairement — on laisse
/// donc son message remonter tel quel plutot que de recoder ce controle.
pub async fn ajouter(repo: &str, branche: &str, creer: bool) -> Result<String, String> {
    let branche = branche.trim();
    if branche.is_empty() {
        return Err("nom de branche vide".into());
    }
    let dossier = dossier_des_worktrees(repo).join(nom_de_dossier(branche)?);
    if dossier.exists() {
        return Err(format!("{} existe déjà", dossier.display()));
    }
    let chemin = dossier.to_string_lossy().to_string();
    let mut args = vec!["worktree", "add"];
    if creer {
        args.push("-b");
        args.push(branche);
        args.push(&chemin);
    } else {
        args.push(&chemin);
        args.push(branche);
    }
    run_git_strict(repo, &args).await?;
    Ok(chemin)
}

/// Retire un worktree. `force` accepte d'abandonner des modifications non validees.
///
/// L'elagage qui suit nettoie ce que git garde en interne quand un dossier a disparu de son
/// cote. Il ne peut rien casser : il ne touche qu'a des references dont le dossier n'existe
/// plus.
pub async fn retirer(repo: &str, chemin: &str, force: bool) -> Result<(), String> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(chemin);
    run_git_strict(repo, &args).await?;
    let _ = run_git_strict(repo, &["worktree", "prune"]).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le format porcelain tel que git le rend, avec les trois cas qui existent : une branche,
    /// une tete detachee, et un worktree verrouille.
    #[test]
    fn analyse_la_sortie_porcelain() {
        let brut = "worktree /home/moi/projet\n\
                    HEAD 1234567890abcdef\n\
                    branch refs/heads/main\n\
                    \n\
                    worktree /home/moi/projet.worktrees/feat-truc\n\
                    HEAD abcdefabcdefabcd\n\
                    branch refs/heads/feat/truc\n\
                    \n\
                    worktree /home/moi/projet.worktrees/essai\n\
                    HEAD 99887766554433\n\
                    detached\n\
                    locked raison quelconque\n";
        let vus = analyser(brut);
        assert_eq!(vus.len(), 3);

        assert_eq!(vus[0].chemin, "/home/moi/projet");
        assert_eq!(vus[0].branche.as_deref(), Some("main"));
        assert_eq!(vus[0].tete, "12345678");
        assert!(vus[0].principal, "le premier bloc est le worktree principal");

        // La branche garde ses `/` : c'est son NOM. Seul le dossier est assaini.
        assert_eq!(vus[1].branche.as_deref(), Some("feat/truc"));
        assert!(!vus[1].principal);

        assert_eq!(vus[2].branche, None, "tete detachee : aucune branche");
        assert!(vus[2].verrouille);
    }

    /// Une sortie vide ne doit pas rendre un worktree fantome.
    #[test]
    fn une_sortie_vide_ne_rend_rien() {
        assert!(analyser("").is_empty());
        assert!(analyser("\n\n").is_empty());
    }

    /// Le dossier est un FRERE du projet, pas un enfant : un worktree dans le depot serait vu
    /// par l'onglet Fichiers et par git comme du contenu non suivi.
    #[test]
    fn le_dossier_des_worktrees_est_frere_du_projet() {
        let d = dossier_des_worktrees("/home/moi/mon-projet");
        assert_eq!(d.to_string_lossy(), "/home/moi/mon-projet.worktrees");
    }

    /// Ce qui pourrait fabriquer une hierarchie ou sortir du dossier est aplati.
    #[test]
    fn un_nom_de_branche_devient_un_nom_de_dossier_sans_surprise() {
        assert_eq!(nom_de_dossier("feat/truc").unwrap(), "feat-truc");
        assert_eq!(nom_de_dossier("release-1.2").unwrap(), "release-1.2");
        assert_eq!(nom_de_dossier("../evasion").unwrap(), "evasion");
        assert_eq!(nom_de_dossier("a b").unwrap(), "a-b");
        // Rien d'utilisable : on refuse au lieu de fabriquer un dossier au nom vide.
        assert!(nom_de_dossier("").is_err());
        assert!(nom_de_dossier("///").is_err());
        assert!(nom_de_dossier("..").is_err());
    }

    /// De bout en bout sur un VRAI depot jetable : ajouter, lister, retirer.
    ///
    /// Les essais sur texte plus haut verifient l'analyse ; celui-ci verifie les APPELS a git,
    /// qui est la seule chose que l'analyse ne peut pas dire. Il demande `git`, qui est une
    /// dependance du projet et est present sur les runners.
    #[test]
    fn le_tour_complet_sur_un_depot_jetable() {
        let base = std::env::temp_dir().join(format!("cockpit-wt-{}", std::process::id()));
        let depot = base.join("projet");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&depot).unwrap();
        let repo = depot.to_string_lossy().to_string();

        // Le menage est fait quoi qu'il arrive : un `assert!` rate laisserait sinon un depot et
        // ses worktrees en travers du dossier temporaire.
        struct Menage(std::path::PathBuf);
        impl Drop for Menage {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _menage = Menage(base.clone());

        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            // Une identite locale au depot : sans elle, `git commit` echoue sur une machine
            // qui n'en a pas de globale — un runner, typiquement.
            for args in [
                vec!["init", "-q"],
                vec!["config", "user.email", "banc@cockpit"],
                vec!["config", "user.name", "Banc"],
                vec!["commit", "-q", "--allow-empty", "-m", "depart"],
            ] {
                run_git_strict(&repo, &args).await.expect("preparation du depot");
            }

            // Au depart, le worktree principal et lui seul.
            let depart = lister(&repo).await.unwrap();
            assert_eq!(depart.len(), 1, "{depart:?}");
            assert!(depart[0].principal);

            // Ajout sur une branche qui n'existe pas : elle est creee.
            let chemin = ajouter(&repo, "feat/essai", true).await.expect("ajout");
            assert!(chemin.ends_with("feat-essai"), "{chemin}");
            assert!(std::path::Path::new(&chemin).is_dir(), "le dossier doit exister");

            let apres = lister(&repo).await.unwrap();
            assert_eq!(apres.len(), 2, "{apres:?}");
            let ajoute = apres.iter().find(|w| !w.principal).unwrap();
            assert_eq!(ajoute.branche.as_deref(), Some("feat/essai"));
            assert_eq!(ajoute.chemin, chemin);

            // Deux fois la meme branche : refuse, et le message vient de git.
            assert!(ajouter(&repo, "feat/essai", true).await.is_err());

            retirer(&repo, &chemin, false).await.expect("retrait");
            let fini = lister(&repo).await.unwrap();
            assert_eq!(fini.len(), 1, "{fini:?}");
            assert!(!std::path::Path::new(&chemin).exists(), "le dossier doit avoir disparu");
        });
    }
}
