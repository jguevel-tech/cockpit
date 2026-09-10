//! Recuperer les preferences d'interface laissees par la version WebKitGTK.
//!
//! **POURQUOI CE MODULE EXISTE.** La base, le fond d'ecran et les terminaux suivent tout
//! seuls : ils vivent dans le dossier de donnees, que les deux versions calculent avec le
//! meme identifiant. Ce qui ne suit pas, c'est le stockage du NAVIGATEUR — Chromium ne lit
//! pas celui de WebKit. Sans ce module, une mise a jour rendrait l'interface en francais
//! par defaut, theme par defaut, zoom a 100 %, et les notices deja lues reapparaitraient.
//!
//! Le fichier de WebKit est une base SQLite a une seule table. On la lit, on ne l'ecrit
//! jamais : la version Tauri peut encore tourner a cet instant.

use std::collections::HashMap;

/// Le nom que WebKitGTK donne au stockage d'une application Tauri. Il est fige : c'est le
/// cas qu'on migre, pas un cas general.
const FICHIER: &str = "localstorage/tauri_localhost_0.localstorage";

/// Les preferences trouvees, par cle. Vide si le fichier n'existe pas — une installation
/// neuve n'a rien a reprendre, et ce n'est pas une erreur.
pub fn lire() -> HashMap<String, String> {
    let Some(dossier) = crate::chemins::dossier_donnees() else {
        log::warn!("preferences heritees : dossier de donnees inconnu");
        return HashMap::new();
    };
    lire_depuis(&dossier.join(FICHIER))
}

/// Separee pour etre eprouvee sur un fichier a soi : un essai qui lirait le VRAI stockage
/// dependrait de la machine qui l'execute.
fn lire_depuis(chemin: &std::path::Path) -> HashMap<String, String> {
    if !chemin.exists() {
        log::info!("preferences heritees : rien a reprendre ({})", chemin.display());
        return HashMap::new();
    }
    // **EN LECTURE SEULE, ET SANS TOUCHER AU JOURNAL.** L'ancienne version peut encore
    // tourner : ouvrir en ecriture creerait un `-wal` a cote du sien et pourrait corrompre
    // ce qu'elle est en train d'ecrire.
    let ouverture = rusqlite::Connection::open_with_flags(
        chemin,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    );
    let base = match ouverture {
        Ok(base) => base,
        Err(e) => {
            log::warn!("preferences heritees : ouverture de {} : {e}", chemin.display());
            return HashMap::new();
        }
    };
    let mut requete = match base.prepare("SELECT key, value FROM ItemTable") {
        Ok(requete) => requete,
        Err(e) => {
            log::warn!("preferences heritees : lecture de la table : {e}");
            return HashMap::new();
        }
    };
    let lignes = requete.query_map([], |l| Ok((l.get::<_, String>(0)?, texte(l.get_ref(1)?))));
    let lignes = match lignes {
        Ok(lignes) => lignes,
        Err(e) => {
            log::warn!("preferences heritees : parcours des lignes : {e}");
            return HashMap::new();
        }
    };
    lignes.filter_map(Result::ok).collect()
}

/// Une valeur du stockage, quelle que soit la facon dont WebKit l'a ecrite.
///
/// **ELLES SONT EN BLOB UTF-16, PAS EN TEXTE**, et c'est ce qui a fait rendre une liste vide
/// a ce module : `get::<String>` refusait chaque ligne avec « Invalid column type Blob ».
/// Les deux formes sont acceptees, une version future pouvant ecrire du texte.
fn texte(valeur: rusqlite::types::ValueRef<'_>) -> String {
    match valeur {
        rusqlite::types::ValueRef::Text(octets) => String::from_utf8_lossy(octets).into_owned(),
        rusqlite::types::ValueRef::Blob(octets) => {
            let paires: Vec<u16> = octets
                .chunks_exact(2)
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect();
            String::from_utf16_lossy(&paires)
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fabriquer(chemin: &std::path::Path, paires: &[(&str, &str)]) {
        let base = rusqlite::Connection::open(chemin).unwrap();
        base.execute("CREATE TABLE ItemTable (key TEXT, value TEXT)", []).unwrap();
        for (cle, valeur) in paires {
            base.execute("INSERT INTO ItemTable VALUES (?1, ?2)", [cle, valeur]).unwrap();
        }
    }

    /// **LE FORMAT REEL DE WEBKIT, ET C'EST TOUT L'INTERET DE CET ESSAI.** Ecrit en TEXT,
    /// il passait alors que le module rendait une liste vide sur la vraie machine.
    #[test]
    fn une_valeur_en_blob_utf16_est_relue() {
        let dossier = std::env::temp_dir().join(format!("cockpit-prefs-blob-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join("stockage.sqlite");
        {
            let base = rusqlite::Connection::open(&chemin).unwrap();
            base.execute("CREATE TABLE ItemTable (key TEXT, value BLOB)", []).unwrap();
            let en_utf16: Vec<u8> = "fr".encode_utf16().flat_map(u16::to_le_bytes).collect();
            base.execute(
                "INSERT INTO ItemTable VALUES ('cockpit-locale', ?1)",
                rusqlite::params![en_utf16],
            )
            .unwrap();
        }
        assert_eq!(
            lire_depuis(&chemin).get("cockpit-locale").map(String::as_str),
            Some("fr")
        );
        let _ = std::fs::remove_dir_all(&dossier);
    }

    #[test]
    fn les_preferences_sont_relues() {
        let dossier = std::env::temp_dir().join(format!("cockpit-prefs-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join("stockage.sqlite");
        fabriquer(&chemin, &[("cockpit-locale", "en"), ("cockpit-theme", "dark")]);

        let lues = lire_depuis(&chemin);
        assert_eq!(lues.get("cockpit-locale").map(String::as_str), Some("en"));
        assert_eq!(lues.get("cockpit-theme").map(String::as_str), Some("dark"));
        let _ = std::fs::remove_dir_all(&dossier);
    }

    /// **UNE INSTALLATION NEUVE N'A RIEN A REPRENDRE, ET CE N'EST PAS UNE PANNE.** Rendre
    /// une erreur ici ferait echouer un demarrage parfaitement normal.
    #[test]
    fn un_fichier_absent_rend_une_liste_vide() {
        assert!(lire_depuis(std::path::Path::new("/n/existe/pas.sqlite")).is_empty());
    }

    /// Un fichier illisible ne doit pas non plus arreter le demarrage : au pire on repart
    /// sur les valeurs par defaut, ce que fait deja une installation neuve.
    #[test]
    fn un_fichier_qui_n_est_pas_une_base_rend_une_liste_vide() {
        let chemin = std::env::temp_dir().join(format!("cockpit-prefs-bidon-{}", std::process::id()));
        std::fs::write(&chemin, b"ceci n'est pas une base SQLite").unwrap();
        assert!(lire_depuis(&chemin).is_empty());
        let _ = std::fs::remove_file(&chemin);
    }
}
