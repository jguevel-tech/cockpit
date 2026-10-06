//! Les branches locales d'un depot : les lister et en renommer une.
//!
//! **LOCALES SEULEMENT** : pas de fetch, pas de `refs/remotes`. Rapide, et c'est la ou l'on
//! travaille ; une branche poussee depuis une autre machine n'y figure pas.

use super::run_git_strict;

/// Les branches de base ne portent jamais de ticket : les compter comme orphelines serait du bruit.
const BASES: &[&str] = &["main", "master", "develop"];

pub async fn lister(depot: &str) -> Result<Vec<String>, String> {
    let sortie = run_git_strict(depot, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]).await?;
    Ok(sortie
        .lines()
        .map(str::trim)
        .filter(|b| !b.is_empty() && !BASES.contains(b))
        .map(str::to_string)
        .collect())
}

async fn existe(depot: &str, branche: &str) -> bool {
    run_git_strict(depot, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branche}")]).await.is_ok()
}

/// Controles d'abord : un refus ne laisse jamais le depot a moitie modifie.
pub async fn renommer(depot: &str, ancienne: &str, nouvelle: &str) -> Result<(), String> {
    if ancienne == nouvelle {
        return Ok(());
    }
    if run_git_strict(depot, &["check-ref-format", "--branch", nouvelle]).await.is_err() {
        return Err(format!("nom de branche invalide : {nouvelle}"));
    }
    if !existe(depot, ancienne).await {
        return Err(format!("la branche {ancienne} n'existe pas"));
    }
    if existe(depot, nouvelle).await {
        return Err(format!("la branche {nouvelle} existe deja"));
    }
    run_git_strict(depot, &["branch", "-m", ancienne, nouvelle]).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn git(dossier: &Path, args: &[&str]) {
        let s = Command::new("git")
            .args(["-c", "user.name=essai", "-c", "user.email=essai@exemple.org", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dossier)
            .output()
            .unwrap();
        assert!(s.status.success(), "git {args:?} : {}", String::from_utf8_lossy(&s.stderr));
    }

    fn depot() -> PathBuf {
        let d = std::env::temp_dir().join(format!("cockpit-branches-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        git(&d, &["init", "-b", "main"]);
        std::fs::write(d.join("a.txt"), "un").unwrap();
        git(&d, &["add", "."]);
        git(&d, &["commit", "-m", "depart"]);
        d
    }

    #[tokio::test]
    async fn liste_sans_les_branches_de_base() {
        let d = depot();
        git(&d, &["branch", "develop"]);
        git(&d, &["branch", "feature/CCM-1/x"]);
        git(&d, &["branch", "essai"]);
        let b = lister(d.to_str().unwrap()).await.unwrap();
        assert_eq!(b, vec!["essai".to_string(), "feature/CCM-1/x".to_string()]);
    }

    #[tokio::test]
    async fn renomme_meme_la_branche_courante() {
        let d = depot();
        git(&d, &["checkout", "-b", "essai"]);
        renommer(d.to_str().unwrap(), "essai", "feature/CCM-2/essai").await.unwrap();
        let b = lister(d.to_str().unwrap()).await.unwrap();
        assert_eq!(b, vec!["feature/CCM-2/essai".to_string()]);
    }

    #[tokio::test]
    async fn refuse_d_ecraser_une_branche() {
        let d = depot();
        git(&d, &["branch", "a"]);
        git(&d, &["branch", "b"]);
        let e = renommer(d.to_str().unwrap(), "a", "b").await.unwrap_err();
        assert!(e.contains("existe deja"), "{e}");
    }

    #[tokio::test]
    async fn refuse_une_branche_absente() {
        let d = depot();
        let e = renommer(d.to_str().unwrap(), "absente", "fix/CCM-3/x").await.unwrap_err();
        assert!(e.contains("n'existe pas"), "{e}");
    }
}
