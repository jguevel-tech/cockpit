//! Personnalisation visuelle : image de fond de l'application.
//!
//! L'image est stockee en FICHIER dans `<app_data>/wallpaper.<ext>`, pas dans la table
//! `settings` : `get_app_settings()` renvoie toutes les cles d'un coup, et y glisser plusieurs
//! centaines de Ko de base64 alourdirait chaque lecture de reglage.
//!
//! Le frontend envoie une data URL deja redimensionnee et re-encodee (voir
//! `stores/appearance.ts`) : c'est lui qui a un canvas, donc lui qui reduit. Ici on se contente
//! de decoder, valider et ecrire.

use base64::Engine;
use std::path::{Path, PathBuf};

/// Plafond apres redimensionnement cote frontend. Large : une image 4K re-encodee en WebP
/// tourne autour de 500 Ko, donc 12 Mo laisse de la marge tout en bornant l'usage disque
/// et memoire (l'image est chargee en base64 a chaque demarrage).
const MAX_BYTES: usize = 12 * 1024 * 1024;

/// Formats acceptes. Volontairement restreint : ce sont les seuls que le WebView rend de
/// facon fiable en `background-image`, et refuser tot vaut mieux qu'un fond invisible.
const ALLOWED: &[(&str, &str)] = &[
    ("image/webp", "webp"),
    ("image/jpeg", "jpg"),
    ("image/png", "png"),
];

fn wallpaper_dir(app_data: &Path) -> PathBuf {
    app_data.to_path_buf()
}

/// Retourne le chemin du fond existant, quel que soit son format.
fn find_existing(app_data: &Path) -> Option<PathBuf> {
    ALLOWED.iter().find_map(|(_, ext)| {
        let p = wallpaper_dir(app_data).join(format!("wallpaper.{}", ext));
        if p.is_file() {
            Some(p)
        } else {
            None
        }
    })
}

/// Decoupe une data URL `data:<mime>;base64,<payload>`.
fn parse_data_url(data_url: &str) -> Result<(&'static str, Vec<u8>), String> {
    let rest = data_url
        .strip_prefix("data:")
        .ok_or_else(|| "format attendu : data:<mime>;base64,<...>".to_string())?;
    let (mime, payload) = rest
        .split_once(";base64,")
        .ok_or_else(|| "encodage base64 attendu dans la data URL".to_string())?;

    let ext = ALLOWED
        .iter()
        .find(|(m, _)| *m == mime)
        .map(|(_, e)| *e)
        .ok_or_else(|| {
            format!(
                "format non supporte : {} (acceptes : {})",
                mime,
                ALLOWED.iter().map(|(m, _)| *m).collect::<Vec<_>>().join(", ")
            )
        })?;

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| format!("base64 invalide : {}", e))?;

    if bytes.is_empty() {
        return Err("image vide".to_string());
    }
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "image trop lourde : {} Mo (maximum {} Mo)",
            bytes.len() / (1024 * 1024),
            MAX_BYTES / (1024 * 1024)
        ));
    }
    Ok((ext, bytes))
}

/// Ecrit le fond et supprime un eventuel fond dans un autre format, pour qu'il n'en reste
/// jamais deux (find_existing prendrait le premier de la liste, pas le plus recent).
pub fn set_wallpaper(app_data: &Path, data_url: &str) -> Result<(), String> {
    let (ext, bytes) = parse_data_url(data_url)?;
    std::fs::create_dir_all(app_data).map_err(|e| format!("mkdir app_data : {}", e))?;

    for (_, other) in ALLOWED.iter().filter(|(_, e)| *e != ext) {
        let _ = std::fs::remove_file(wallpaper_dir(app_data).join(format!("wallpaper.{}", other)));
    }

    let path = wallpaper_dir(app_data).join(format!("wallpaper.{}", ext));
    std::fs::write(&path, &bytes).map_err(|e| format!("ecriture {} : {}", path.display(), e))
}

/// Relit le fond sous forme de data URL, prete a etre posee en `background-image`.
pub fn get_wallpaper(app_data: &Path) -> Result<Option<String>, String> {
    let Some(path) = find_existing(app_data) else {
        return Ok(None);
    };
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mime = ALLOWED
        .iter()
        .find(|(_, e)| *e == ext)
        .map(|(m, _)| *m)
        .ok_or_else(|| format!("extension inattendue : {}", ext))?;

    let bytes = std::fs::read(&path).map_err(|e| format!("lecture {} : {}", path.display(), e))?;
    let payload = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:{};base64,{}", mime, payload)))
}

/// Lit une image du disque et la renvoie en data URL, pour que le frontend puisse la charger
/// dans un canvas (redimensionnement + extraction de la couleur dominante).
///
/// Fait en Rust plutot que via `@tauri-apps/plugin-fs` : ce plugin n'est pas installe cote JS,
/// et l'ajouter impliquerait un jeu de permissions de lecture bien plus large que « une image
/// choisie par l'utilisateur ». Ici la validation est explicite et le perimetre minimal.
pub fn read_image_as_data_url(path: &str) -> Result<String, String> {
    let p = Path::new(path);
    if !p.is_file() {
        return Err(format!("fichier introuvable : {}", path));
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        other => return Err(format!("format non supporte : .{}", other)),
    };

    let meta = std::fs::metadata(p).map_err(|e| format!("lecture metadonnees : {}", e))?;
    // Borne AVANT de lire : une image de 200 Mo choisie par erreur ne doit pas etre chargee
    // en memoire puis en base64 (x1.33) juste pour etre refusee ensuite.
    const MAX_SOURCE: u64 = 40 * 1024 * 1024;
    if meta.len() > MAX_SOURCE {
        return Err(format!(
            "image trop lourde : {} Mo (maximum {} Mo)",
            meta.len() / (1024 * 1024),
            MAX_SOURCE / (1024 * 1024)
        ));
    }

    let bytes = std::fs::read(p).map_err(|e| format!("lecture {} : {}", path, e))?;
    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}

pub fn clear_wallpaper(app_data: &Path) -> Result<(), String> {
    for (_, ext) in ALLOWED {
        let p = wallpaper_dir(app_data).join(format!("wallpaper.{}", ext));
        if p.is_file() {
            std::fs::remove_file(&p).map_err(|e| format!("suppression {} : {}", p.display(), e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1x1 PNG transparent, le plus petit PNG valide.
    const PNG_1PX: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYGD4DwABBAEAX+XKPQAAAABJRU5ErkJggg==";

    #[test]
    fn ecrit_relit_et_supprime() {
        let dir = tempdir();
        assert!(get_wallpaper(&dir).unwrap().is_none(), "rien au depart");

        set_wallpaper(&dir, PNG_1PX).unwrap();
        let read = get_wallpaper(&dir).unwrap().expect("fond present");
        assert!(read.starts_with("data:image/png;base64,"));
        assert_eq!(read, PNG_1PX, "aller-retour sans perte");

        clear_wallpaper(&dir).unwrap();
        assert!(get_wallpaper(&dir).unwrap().is_none(), "supprime");
    }

    #[test]
    fn un_seul_fond_a_la_fois() {
        let dir = tempdir();
        // Un PNG puis un JPEG : le PNG doit disparaitre, sinon find_existing renverrait
        // toujours le webp/jpg selon l'ordre de la liste et jamais le dernier choisi.
        set_wallpaper(&dir, PNG_1PX).unwrap();
        let jpeg = format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode([0xFF, 0xD8, 0xFF, 0xD9]));
        set_wallpaper(&dir, &jpeg).unwrap();

        assert!(!dir.join("wallpaper.png").exists(), "ancien format supprime");
        assert!(dir.join("wallpaper.jpg").exists());
    }

    #[test]
    fn refuse_les_entrees_invalides() {
        let dir = tempdir();
        assert!(set_wallpaper(&dir, "pas une data url").is_err());
        assert!(set_wallpaper(&dir, "data:image/gif;base64,AAAA").is_err(), "format non supporte");
        assert!(set_wallpaper(&dir, "data:image/png;base64,***").is_err(), "base64 invalide");
        assert!(set_wallpaper(&dir, "data:image/png;base64,").is_err(), "image vide");
    }

    fn tempdir() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "cockpit-wallpaper-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
