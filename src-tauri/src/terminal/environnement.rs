//! L'environnement a donner a un shell lance par Cockpit.
//!
//! Rien ici ne connait tmux : c'est vrai de tout serveur de terminaux, y compris du
//! service maison (`terminal/service/`). Extrait de `tmux.rs` a l'etape B2 du chantier
//! (`docs/portabilite/plan-terminaux.md`) parce que le service doit lancer les shells
//! lui-meme, avec EXACTEMENT le meme nettoyage — le dupliquer aurait garanti qu'une des
//! deux copies derive.

/// Variables du runtime AppImage a NE PAS transmettre aux shells.
///
/// L'AppImage pose PYTHONHOME/PYTHONPATH/LD_LIBRARY_PATH... pointant dans son montage
/// /tmp/.mount_cockpi*. Le serveur de terminaux etant lance par Cockpit, chaque shell les
/// heritait : `python3` plantait dans TOUS les terminaux Cockpit (« ModuleNotFoundError:
/// encodings », constate le 2026-08-13), et LD_LIBRARY_PATH pouvait derregler n'importe
/// quel binaire.
pub const VARIABLES_APPIMAGE: &[&str] = &[
    "PYTHONHOME", "PYTHONPATH", "LD_LIBRARY_PATH", "LD_PRELOAD",
    "APPDIR", "APPIMAGE", "APPIMAGE_ORIGINAL_APPDIR", "ARGV0", "OWD", "PERLLIB",
    // Injectees par le hook GTK du bundle (verifiees dans l'AppImage publiee) : elles
    // designent le montage /tmp/.mount_cockpi*, qui disparait et n'a rien a voir avec
    // l'environnement de l'utilisateur.
    "GTK_PATH", "GTK_EXE_PREFIX", "GTK_DATA_PREFIX", "GTK_IM_MODULE_FILE", "GTK_THEME",
    "GDK_PIXBUF_MODULE_FILE", "GDK_BACKEND",
    "GIO_MODULE_DIR", "GIO_EXTRA_MODULES", "GSETTINGS_SCHEMA_DIR", "GI_TYPELIB_PATH",
    "GST_PLUGIN_SYSTEM_PATH", "GST_PLUGIN_SYSTEM_PATH_1_0", "GST_PLUGIN_PATH",
    "GST_PLUGIN_PATH_1_0", "GST_PLUGIN_SCANNER", "GST_PLUGIN_SCANNER_1_0",
    // Notre propre configuration de polices (voir lib.rs) : elle sert au rendu de
    // l'interface, pas aux programmes lances dans un terminal.
    "FONTCONFIG_FILE", "FONTCONFIG_PATH",
];

/// Variables qui sont des LISTES de chemins : on ne les supprime pas — le shell et les
/// outils en ont besoin — on en retire seulement les entrees situees dans le montage de
/// l'AppImage.
const LISTES_DE_CHEMINS: &[&str] = &["XDG_DATA_DIRS", "XDG_CONFIG_DIRS", "PATH"];

/// Retire d'une liste `:` les chemins situes sous `appdir`.
///
/// Un utilisateur a signale des erreurs de `mise` dans les terminaux de Cockpit : ce genre
/// d'outil lit XDG_DATA_DIRS et le PATH, et l'AppImage y ajoutait son propre montage. Ce
/// n'etait donc ni son installation ni un probleme de `mise`.
fn sans_chemins_appimage(valeur: &str, appdir: &str) -> String {
    valeur
        .split(':')
        .filter(|part| !part.is_empty() && !part.starts_with(appdir))
        .collect::<Vec<_>>()
        .join(":")
}

/// Ce qu'il faut changer a l'environnement avant de lancer une commande destinee a
/// l'utilisateur (shell, tmux) : les variables a RETIRER, et celles a REDEFINIR.
///
/// Rend les deux listes au lieu d'agir directement : l'appelant les applique a sa commande,
/// et la decision reste testable sans lancer de processus.
pub fn modifications(
    appdir: Option<&str>,
    lire: &dyn Fn(&str) -> Option<String>,
) -> (Vec<&'static str>, Vec<(&'static str, String)>) {
    let mut retirer: Vec<&'static str> = VARIABLES_APPIMAGE.to_vec();
    let mut redefinir: Vec<(&'static str, String)> = Vec::new();
    let Some(appdir) = appdir.filter(|d| !d.is_empty()) else {
        return (retirer, redefinir);
    };
    for var in LISTES_DE_CHEMINS {
        if let Some(valeur) = lire(var) {
            let propre = sans_chemins_appimage(&valeur, appdir);
            if propre.is_empty() {
                retirer.push(var);
            } else if propre != valeur {
                redefinir.push((var, propre));
            }
        }
    }
    (retirer, redefinir)
}

/// Les memes modifications, calculees depuis l'environnement du processus.
pub fn modifications_courantes() -> (Vec<&'static str>, Vec<(&'static str, String)>) {
    let appdir = std::env::var("APPDIR").ok();
    modifications(appdir.as_deref(), &|v| std::env::var(v).ok())
}

/// Locale UTF-8 a imposer aux terminaux : celle de l'utilisateur si elle est
/// deja en UTF-8, sinon un repli garanti disponible.
///
/// Lance depuis un `.desktop`, Cockpit peut heriter d'un environnement sans LANG — un
/// terminal compte alors chaque octet UTF-8 comme une colonne, et les accents decalent
/// tout ce qui suit.
pub fn locale_utf8() -> String {
    for var in ["LC_ALL", "LC_CTYPE", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if v.to_lowercase().contains("utf") {
                return v;
            }
        }
    }
    // Replis courants (au moins un existe sur toute distro moderne)
    "C.UTF-8".to_string()
}

/// Applique le nettoyage a une commande systeme.
pub fn appliquer(cmd: &mut std::process::Command) {
    let (retirer, redefinir) = modifications_courantes();
    for var in retirer {
        cmd.env_remove(var);
    }
    for (var, valeur) in redefinir {
        cmd.env(var, valeur);
    }
}

/// Applique le nettoyage a une commande lancee dans un PTY, et pose la locale UTF-8.
pub fn appliquer_pty(cmd: &mut portable_pty::CommandBuilder) {
    let (retirer, redefinir) = modifications_courantes();
    for var in retirer {
        cmd.env_remove(var);
    }
    for (var, valeur) in redefinir {
        cmd.env(var, valeur);
    }
    cmd.env("LANG", locale_utf8());
    cmd.env("LC_ALL", locale_utf8());
}

#[cfg(test)]
mod tests {
    use super::modifications;

    #[test]
    fn les_variables_du_montage_appimage_sont_retirees() {
        let (retirer, _) = modifications(None, &|_| None);
        for attendue in ["PYTHONHOME", "LD_LIBRARY_PATH", "GIO_EXTRA_MODULES", "GI_TYPELIB_PATH"] {
            assert!(retirer.contains(&attendue), "{attendue} devrait etre retiree");
        }
    }

    #[test]
    fn gst_est_retire_car_il_masquait_le_gstreamer_du_systeme() {
        let (retirer, _) = modifications(None, &|_| None);
        assert!(retirer.contains(&"GST_PLUGIN_SYSTEM_PATH_1_0"));
    }

    #[test]
    fn les_listes_de_chemins_perdent_seulement_les_entrees_du_montage() {
        // XDG_DATA_DIRS ne doit PAS disparaitre : le shell et les outils s'en servent.
        // Seule l'entree pointant dans le montage de l'AppImage s'en va.
        let lire = |v: &str| match v {
            "XDG_DATA_DIRS" => Some("/tmp/.mount_ck1/usr/share:/usr/share:/usr/local/share".to_string()),
            _ => None,
        };
        let (_, redefinir) = modifications(Some("/tmp/.mount_ck1"), &lire);
        let valeur = redefinir
            .iter()
            .find(|(k, _)| *k == "XDG_DATA_DIRS")
            .map(|(_, v)| v.clone())
            .expect("XDG_DATA_DIRS devrait etre redefinie");
        assert_eq!(valeur, "/usr/share:/usr/local/share");
    }

    #[test]
    fn une_liste_entierement_dans_le_montage_est_retiree() {
        let lire = |v: &str| match v {
            "XDG_CONFIG_DIRS" => Some("/tmp/.mount_ck1/etc/xdg".to_string()),
            _ => None,
        };
        let (retirer, redefinir) = modifications(Some("/tmp/.mount_ck1"), &lire);
        assert!(retirer.contains(&"XDG_CONFIG_DIRS"));
        assert!(redefinir.iter().all(|(k, _)| *k != "XDG_CONFIG_DIRS"));
    }

    #[test]
    fn hors_appimage_les_listes_ne_sont_pas_touchees() {
        let lire = |v: &str| match v {
            "PATH" => Some("/usr/bin:/bin".to_string()),
            _ => None,
        };
        let (_, redefinir) = modifications(None, &lire);
        assert!(redefinir.is_empty(), "{redefinir:?}");
    }

    #[test]
    fn le_path_garde_les_entrees_de_l_utilisateur() {
        // Cas de l'utilisateur dont `mise` echouait : ses raccourcis sont dans son PATH,
        // il ne faut surtout pas le vider.
        let lire = |v: &str| match v {
            "PATH" => Some("/tmp/.mount_ck1/usr/bin:/home/moi/.local/share/mise/shims:/usr/bin".to_string()),
            _ => None,
        };
        let (_, redefinir) = modifications(Some("/tmp/.mount_ck1"), &lire);
        let path = redefinir.iter().find(|(k, _)| *k == "PATH").map(|(_, v)| v.clone()).unwrap();
        assert_eq!(path, "/home/moi/.local/share/mise/shims:/usr/bin");
    }
}
