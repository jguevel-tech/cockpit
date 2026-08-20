//! Explorateur de fichiers du projet : listing lazy gitignore-aware + lecture de fichiers.

pub mod claude_sessions;

use ignore::WalkBuilder;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Au-dela de cette taille, le contenu est tronque (viewer, pas editeur de gros fichiers).
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Serialize, Clone)]
pub struct DirEntry {
    pub name: String,
    pub rel_path: String,
    pub is_dir: bool,
}

#[derive(Serialize, Clone)]
pub struct FileContent {
    pub content: String,
    pub size: u64,
    pub truncated: bool,
    pub binary: bool,
    /// Date de modification en MILLISECONDES depuis epoch (0 si indisponible).
    /// En millisecondes et pas en nanosecondes : le frontend recoit un `number`
    /// JSON, et des nanosecondes depassent la precision entiere de JS (2^53).
    pub mtime: u64,
}

/// Etat disque d'un fichier, sans lire son contenu : c'est ce qui permet de
/// surveiller le fichier affiche pour trois sous et de ne relire que s'il a bouge.
#[derive(Serialize, Clone)]
pub struct FileStat {
    pub size: u64,
    pub mtime: u64,
}

/// mtime en millisecondes depuis epoch, 0 si la plateforme ne la donne pas.
fn mtime_millis(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Joint `rel` a la racine du projet en interdisant de sortir de la racine.
fn secure_join(root: &str, rel: &str) -> Result<(PathBuf, PathBuf), String> {
    let root = Path::new(root)
        .canonicalize()
        .map_err(|e| format!("racine projet: {}", e))?;
    let joined = if rel.is_empty() { root.clone() } else { root.join(rel) };
    let resolved = joined
        .canonicalize()
        .map_err(|e| format!("chemin {}: {}", rel, e))?;
    if !resolved.starts_with(&root) {
        return Err("chemin hors du projet".into());
    }
    Ok((root, resolved))
}

/// Liste un repertoire (non recursif) en respectant .gitignore, comme l'arbre de Warp.
pub fn list_dir(project_path: &str, rel_path: &str) -> Result<Vec<DirEntry>, String> {
    let (root, dir) = secure_join(project_path, rel_path)?;
    if !dir.is_dir() {
        return Err("pas un repertoire".into());
    }

    let mut entries: Vec<DirEntry> = WalkBuilder::new(&dir)
        .max_depth(Some(1))
        .hidden(false) // on montre les fichiers caches (.env, .gitlab-ci.yml...)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.depth() == 1)
        .filter_map(|e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let rel = e.path().strip_prefix(&root).ok()?.to_string_lossy().to_string();
            Some(DirEntry {
                name: e.file_name().to_string_lossy().to_string(),
                rel_path: rel,
                is_dir,
            })
        })
        .collect();

    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

pub fn read_project_file(project_path: &str, rel_path: &str) -> Result<FileContent, String> {
    let (_, path) = secure_join(project_path, rel_path)?;
    if !path.is_file() {
        return Err("pas un fichier".into());
    }

    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let size = meta.len();
    let mtime = mtime_millis(&meta);
    let truncated = size > MAX_FILE_BYTES;

    let bytes = if truncated {
        use std::io::Read;
        let mut buf = vec![0u8; MAX_FILE_BYTES as usize];
        let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        buf.truncate(n);
        buf
    } else {
        std::fs::read(&path).map_err(|e| e.to_string())?
    };

    let binary = bytes.iter().take(8192).any(|&b| b == 0);
    let content = if binary {
        String::new()
    } else {
        String::from_utf8_lossy(&bytes).to_string()
    };

    Ok(FileContent { content, size, truncated, binary, mtime })
}

/// Etat disque du fichier AFFICHE, pour detecter une modification exterieure
/// (le cas courant ici : un agent qui edite le fichier dans un terminal Cockpit
/// pendant qu'on le regarde dans l'onglet Fichiers).
///
/// `Ok(None)` = le fichier n'existe plus. Ce n'est pas une erreur a remonter
/// comme une panne : l'appelant l'affiche comme un etat. Toute AUTRE erreur
/// (droits, racine projet introuvable) reste une `Err`.
pub fn stat_project_file(project_path: &str, rel_path: &str) -> Result<Option<FileStat>, String> {
    let (root, _) = secure_join(project_path, "")?;
    if rel_path.is_empty() {
        return Err("chemin vide".into());
    }
    // Traversee refusee AVANT toute canonicalisation. Le controle par le parent
    // canonicalise (plus bas) ne voit une remontee que si le dossier vise EXISTE : sinon
    // canonicalize rend NotFound et on repondait Ok(None), c'est-a-dire « pas de fichier »
    // au lieu de « chemin interdit ». La garde dependait donc de la disposition du disque.
    // Constate en CI sur macOS : depuis /var/folders/<hash>/T/, « ../../etc » vise un
    // dossier inexistant, la ou sous Linux depuis /tmp il vise /etc, qui existe.
    if Path::new(rel_path)
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_)))
    {
        return Err("chemin hors du projet".into());
    }
    let joined = root.join(rel_path);
    let leaf = joined.file_name().ok_or("chemin sans nom de fichier")?.to_owned();
    // secure_join ne peut pas canonicaliser un chemin inexistant : on canonicalise
    // le PARENT et on verifie qu'il est dans la racine. Parent disparu = fichier disparu.
    let parent = match joined.parent().ok_or("chemin sans parent")?.canonicalize() {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !parent.starts_with(&root) {
        return Err("chemin hors du projet".into());
    }
    match std::fs::metadata(parent.join(leaf)) {
        Ok(m) if m.is_file() => Ok(Some(FileStat { size: m.len(), mtime: mtime_millis(&m) })),
        // Remplace par un dossier : plus le fichier qu'on affichait.
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Ecrit un fichier EXISTANT du projet (editeur de l'onglet Fichiers).
/// Chemin verrouille a la racine comme la lecture ; pas de creation ici.
pub fn write_project_file(project_path: &str, rel_path: &str, content: &str) -> Result<(), String> {
    let (_, path) = secure_join(project_path, rel_path)?;
    if !path.is_file() {
        return Err("pas un fichier".into());
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[derive(Serialize, Clone)]
pub struct SymbolHit {
    pub rel_path: String,
    /// 0-indexee (convention LSP, comme le frontend)
    pub line: u32,
    pub preview: String,
}

const SYMBOL_MAX_HITS: usize = 30;
const SYMBOL_MAX_FILE: u64 = 512 * 1024;
const CODE_EXTS: &[&str] = &[
    "php", "rs", "ts", "js", "mjs", "cjs", "tsx", "jsx", "svelte", "vue",
    "py", "rb", "go", "java", "kt", "c", "h", "cpp", "hpp", "cs", "swift", "twig",
];

/// Recherche heuristique de DECLARATIONS d'un symbole dans le projet
/// (repli quand aucun serveur LSP n'est disponible pour le langage).
/// Couvre les formes classiques : class/interface/trait/enum/struct/type X,
/// function/fn/def/func X(, const X =, X = function/arrow.
pub fn find_symbol(project_path: &str, symbol: &str) -> Result<Vec<SymbolHit>, String> {
    let (root, _) = secure_join(project_path, "")?;
    if symbol.is_empty() || !symbol.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("symbole invalide".into());
    }

    let patterns = [
        format!(r"^\s*(?:export\s+)?(?:abstract\s+|final\s+)?(?:class|interface|trait|enum|struct|type)\s+{}\b", symbol),
        format!(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:public\s+|private\s+|protected\s+|static\s+)*(?:function|fn|def|func)\s+&?{}\s*[(<]", symbol),
        format!(r"^\s*(?:export\s+)?(?:public\s+|private\s+|protected\s+)?const\s+{}\b", symbol),
        format!(r"^\s*(?:export\s+)?(?:const|let|var)\s+{}\s*=", symbol),
    ];
    let regexes: Vec<regex::Regex> = patterns
        .iter()
        .filter_map(|p| regex::Regex::new(p).ok())
        .collect();

    let mut hits = Vec::new();
    let walker = WalkBuilder::new(&root).hidden(true).build();
    'files: for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !CODE_EXTS.contains(&ext) {
            continue;
        }
        if entry.metadata().map(|m| m.len() > SYMBOL_MAX_FILE).unwrap_or(true) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else { continue };
        for (i, line) in text.lines().enumerate() {
            if regexes.iter().any(|r| r.is_match(line)) {
                hits.push(SymbolHit {
                    rel_path: path.strip_prefix(&root).unwrap_or(path).to_string_lossy().to_string(),
                    line: i as u32,
                    preview: line.trim().chars().take(120).collect(),
                });
                if hits.len() >= SYMBOL_MAX_HITS {
                    break 'files;
                }
            }
        }
    }
    Ok(hits)
}

// --- Apercu d'images (onglet Fichiers) ---

const IMAGE_MAX_BYTES: u64 = 10 * 1024 * 1024;

fn image_mime(ext: &str) -> Option<&'static str> {
    Some(match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        _ => return None,
    })
}

/// Lit une image du projet en data URL pour l'apercu (racine verrouillee, 10 Mo max).
pub fn read_project_image(project_path: &str, rel_path: &str) -> Result<String, String> {
    use base64::Engine;
    let (_, path) = secure_join(project_path, rel_path)?;
    if !path.is_file() {
        return Err("pas un fichier".into());
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let mime = image_mime(ext).ok_or("pas une image")?;
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size > IMAGE_MAX_BYTES {
        return Err("image trop lourde pour l'aperçu (10 Mo max)".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(format!(
        "data:{};base64,{}",
        mime,
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

// --- Gestion de fichiers (creation, renommage, suppression) ---

/// Valide un nom de FEUILLE (fichier ou dossier) : pas de separateur, pas de traversee.
/// C'est le complement de secure_join, qui ne peut pas canonicaliser un chemin inexistant.
fn validate_leaf_name(name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("nom vide".into());
    }
    if name == "." || name == ".." || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(format!("nom invalide: {}", name));
    }
    Ok(())
}

/// Cree un fichier VIDE dans un repertoire existant du projet. Refuse d'ecraser.
pub fn create_project_file(project_path: &str, rel_dir: &str, name: &str) -> Result<String, String> {
    validate_leaf_name(name)?;
    let (root, dir) = secure_join(project_path, rel_dir)?;
    if !dir.is_dir() {
        return Err("pas un repertoire".into());
    }
    let target = dir.join(name.trim());
    if target.exists() {
        return Err(format!("{} existe déjà", name.trim()));
    }
    std::fs::write(&target, "").map_err(|e| e.to_string())?;
    Ok(target.strip_prefix(&root).unwrap_or(&target).to_string_lossy().to_string())
}

/// Cree un sous-dossier dans un repertoire existant du projet. Refuse d'ecraser.
pub fn create_project_dir(project_path: &str, rel_dir: &str, name: &str) -> Result<String, String> {
    validate_leaf_name(name)?;
    let (root, dir) = secure_join(project_path, rel_dir)?;
    if !dir.is_dir() {
        return Err("pas un repertoire".into());
    }
    let target = dir.join(name.trim());
    if target.exists() {
        return Err(format!("{} existe déjà", name.trim()));
    }
    std::fs::create_dir(&target).map_err(|e| e.to_string())?;
    Ok(target.strip_prefix(&root).unwrap_or(&target).to_string_lossy().to_string())
}

/// Renomme un fichier ou dossier DANS son repertoire (pas un deplacement). Refuse d'ecraser.
pub fn rename_project_entry(project_path: &str, rel_path: &str, new_name: &str) -> Result<String, String> {
    validate_leaf_name(new_name)?;
    let (root, path) = secure_join(project_path, rel_path)?;
    if path == root {
        return Err("impossible de renommer la racine du projet".into());
    }
    let parent = path.parent().ok_or("chemin sans parent")?;
    let target = parent.join(new_name.trim());
    if target.exists() {
        return Err(format!("{} existe déjà", new_name.trim()));
    }
    std::fs::rename(&path, &target).map_err(|e| e.to_string())?;
    Ok(target.strip_prefix(&root).unwrap_or(&target).to_string_lossy().to_string())
}

/// Envoie un fichier ou dossier a la CORBEILLE SYSTEME — jamais de suppression
/// definitive depuis l'UI : une erreur de clic doit rester rattrapable.
pub fn trash_project_entry(project_path: &str, rel_path: &str) -> Result<(), String> {
    let (root, path) = secure_join(project_path, rel_path)?;
    if path == root {
        return Err("impossible de supprimer la racine du projet".into());
    }
    trash::delete(&path).map_err(|e| format!("mise à la corbeille impossible : {}", e))
}

// --- Recherche globale (style IDE : noms de dossiers/fichiers + contenu) ---

#[derive(Serialize, Clone)]
pub struct NameHit {
    pub rel_path: String,
    pub is_dir: bool,
}

#[derive(Serialize, Clone)]
pub struct ContentHit {
    pub rel_path: String,
    /// 0-indexee (meme convention que goto_definition / scrollToLine du frontend)
    pub line: u32,
    pub preview: String,
}

#[derive(Serialize, Clone)]
pub struct SearchResults {
    pub names: Vec<NameHit>,
    pub contents: Vec<ContentHit>,
    /// Des resultats ont ete coupes : inviter a preciser la recherche.
    pub truncated: bool,
}

const SEARCH_MAX_NAME_HITS: usize = 100;
const SEARCH_MAX_CONTENT_HITS: usize = 400;

/// Recherche insensible a la casse dans les NOMS (fichiers et dossiers) et le CONTENU
/// des fichiers texte du projet. gitignore respecte, binaires et fichiers > 2 Mo ignores.
/// Sous-chaine simple, pas de regex : c'est une recherche d'utilisateur, pas de machine.
pub fn search_project(project_path: &str, query: &str) -> Result<SearchResults, String> {
    let (root, _) = secure_join(project_path, "")?;
    let q = query.trim().to_lowercase();
    if q.chars().count() < 2 {
        return Err("recherche trop courte (2 caractères minimum)".into());
    }

    let mut names = Vec::new();
    let mut contents = Vec::new();
    let mut truncated = false;

    let walker = WalkBuilder::new(&root)
        .hidden(false)
        .filter_entry(|e| e.file_name() != ".git")
        .build();
    for entry in walker.filter_map(|e| e.ok()) {
        if entry.depth() == 0 {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(&root).unwrap_or(path).to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

        if entry.file_name().to_string_lossy().to_lowercase().contains(&q) {
            if names.len() < SEARCH_MAX_NAME_HITS {
                names.push(NameHit { rel_path: rel.clone(), is_dir });
            } else {
                truncated = true;
            }
        }

        if is_dir || contents.len() >= SEARCH_MAX_CONTENT_HITS {
            continue;
        }
        if entry.metadata().map(|m| m.len() > MAX_FILE_BYTES).unwrap_or(true) {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else { continue };
        if bytes.iter().take(8192).any(|&b| b == 0) {
            continue; // binaire
        }
        let text = String::from_utf8_lossy(&bytes);
        for (i, line) in text.lines().enumerate() {
            if line.to_lowercase().contains(&q) {
                contents.push(ContentHit {
                    rel_path: rel.clone(),
                    line: i as u32,
                    preview: line.trim().chars().take(200).collect(),
                });
                if contents.len() >= SEARCH_MAX_CONTENT_HITS {
                    truncated = true;
                    break;
                }
            }
        }
    }

    Ok(SearchResults { names, contents, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_dir_respects_gitignore_and_sorts() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::create_dir_all(dir.join("node_modules")).unwrap();
        std::fs::write(dir.join(".gitignore"), "node_modules/\n").unwrap();
        std::fs::write(dir.join("a.txt"), "hello").unwrap();

        let entries = list_dir(dir.to_str().unwrap(), "").unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"src"));
        assert!(names.contains(&"a.txt"));
        assert!(!names.contains(&".git"));
        assert!(!names.contains(&"node_modules"), "gitignore doit etre respecte: {:?}", names);
        // Dossiers d'abord
        assert!(entries[0].is_dir);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_file_and_path_escape() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_read_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), "contenu").unwrap();

        let f = read_project_file(dir.to_str().unwrap(), "f.txt").unwrap();
        assert_eq!(f.content, "contenu");
        assert!(!f.binary && !f.truncated);

        assert!(read_project_file(dir.to_str().unwrap(), "../../../etc/passwd").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_write_project_file() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_write_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.txt"), "avant").unwrap();

        write_project_file(dir.to_str().unwrap(), "f.txt", "apres").unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "apres");
        // Pas de creation de nouveaux fichiers ni d'evasion
        assert!(write_project_file(dir.to_str().unwrap(), "nouveau.txt", "x").is_err());
        assert!(write_project_file(dir.to_str().unwrap(), "../evil.txt", "x").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_read_project_image() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_img_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("pixel.png"), [0x89, 0x50, 0x4E, 0x47]).unwrap();
        std::fs::write(dir.join("code.rs"), "fn main() {}").unwrap();
        let root = dir.to_str().unwrap();

        let url = read_project_image(root, "pixel.png").unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        // Pas une image : refuse
        assert!(read_project_image(root, "code.rs").is_err());
        // Evasion : refuse
        assert!(read_project_image(root, "../pixel.png").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_gestion_fichiers_creation_renommage_gardes() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_mgmt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        let root = dir.to_str().unwrap();

        // Creation de fichier et de dossier
        let rel = create_project_file(root, "src", "notes.md").unwrap();
        assert_eq!(rel, "src/notes.md");
        assert!(dir.join("src/notes.md").is_file());
        let rel = create_project_dir(root, "", "docs").unwrap();
        assert_eq!(rel, "docs");

        // Refus d'ecraser
        assert!(create_project_file(root, "src", "notes.md").is_err());
        // Noms invalides et traversee
        assert!(create_project_file(root, "", "a/b").is_err());
        assert!(create_project_file(root, "", "..").is_err());
        assert!(create_project_file(root, "../..", "evil").is_err());

        // Renommage dans le meme dossier, sans ecrasement
        let rel = rename_project_entry(root, "src/notes.md", "idees.md").unwrap();
        assert_eq!(rel, "src/idees.md");
        assert!(dir.join("src/idees.md").is_file());
        std::fs::write(dir.join("src/autre.md"), "x").unwrap();
        assert!(rename_project_entry(root, "src/idees.md", "autre.md").is_err());
        // La racine est intouchable
        assert!(rename_project_entry(root, "", "autre_nom").is_err());
        assert!(trash_project_entry(root, "").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search_project_noms_contenu_et_bornes() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_search_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("SousDossierAlpha")).unwrap();
        std::fs::create_dir_all(dir.join("ignore_moi")).unwrap();
        // Le crate `ignore` n'applique le .gitignore QUE dans un depot git
        std::fs::create_dir_all(dir.join(".git")).unwrap();
        std::fs::write(dir.join(".gitignore"), "ignore_moi/\n").unwrap();
        std::fs::write(dir.join("alpha_notes.txt"), "premiere ligne\nvoici ALPHA en majuscules\n").unwrap();
        std::fs::write(dir.join("autre.txt"), "rien ici\n").unwrap();
        std::fs::write(dir.join("ignore_moi/alpha.txt"), "alpha ignore\n").unwrap();
        std::fs::write(dir.join("binaire.bin"), b"alpha\x00binaire").unwrap();

        let res = search_project(dir.to_str().unwrap(), "alpha").unwrap();

        // Noms : le dossier ET le fichier matchent, insensible a la casse ;
        // rien depuis ignore_moi (gitignore) ni depuis le binaire (nom sans "alpha"... le .bin matche pas)
        let name_paths: Vec<&str> = res.names.iter().map(|n| n.rel_path.as_str()).collect();
        assert!(name_paths.contains(&"SousDossierAlpha"), "{:?}", name_paths);
        assert!(name_paths.contains(&"alpha_notes.txt"));
        assert!(!name_paths.iter().any(|p| p.starts_with("ignore_moi")));

        // Contenu : la ligne en MAJUSCULES matche, le binaire est saute
        assert_eq!(res.contents.len(), 1, "{:?}", res.contents.iter().map(|c| &c.rel_path).collect::<Vec<_>>());
        assert_eq!(res.contents[0].rel_path, "alpha_notes.txt");
        assert_eq!(res.contents[0].line, 1);
        assert!(res.contents[0].preview.contains("ALPHA"));
        assert!(!res.truncated);

        // Requete trop courte refusee
        assert!(search_project(dir.to_str().unwrap(), "a").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_symbol_declarations() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_sym_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/a.php"),
            "<?php\nclass ManifestManager {\n  public function loadIfExists($id) {}\n}\n$x = new ManifestManager();\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/b.ts"), "export const SITE_IDS = ['a'];\nuse(SITE_IDS);\n").unwrap();

        let hits = find_symbol(dir.to_str().unwrap(), "ManifestManager").unwrap();
        assert_eq!(hits.len(), 1, "declaration seulement, pas l'usage: {:?}", hits.iter().map(|h| &h.preview).collect::<Vec<_>>());
        assert_eq!(hits[0].line, 1);

        let hits = find_symbol(dir.to_str().unwrap(), "loadIfExists").unwrap();
        assert_eq!(hits.len(), 1);

        let hits = find_symbol(dir.to_str().unwrap(), "SITE_IDS").unwrap();
        assert_eq!(hits.len(), 1);

        assert!(find_symbol(dir.to_str().unwrap(), "a; DROP").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stat_detecte_modification_exterieure() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_stat_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sous")).unwrap();
        std::fs::write(dir.join("sous/f.txt"), "avant").unwrap();
        let root = dir.to_str().unwrap();

        let ouvert = read_project_file(root, "sous/f.txt").unwrap();
        assert!(ouvert.mtime > 0, "read_project_file doit rendre une mtime exploitable");

        let st = stat_project_file(root, "sous/f.txt").unwrap().expect("fichier present");
        assert_eq!(st.mtime, ouvert.mtime, "meme fichier intact = meme mtime");
        assert_eq!(st.size, ouvert.size);

        // Un agent reecrit le fichier pendant qu'on l'affiche.
        std::thread::sleep(std::time::Duration::from_millis(15));
        std::fs::write(dir.join("sous/f.txt"), "apres modification exterieure").unwrap();
        let st2 = stat_project_file(root, "sous/f.txt").unwrap().expect("fichier present");
        assert_ne!(st2.mtime, ouvert.mtime, "la modification exterieure doit se voir");
        assert_eq!(read_project_file(root, "sous/f.txt").unwrap().content, "apres modification exterieure");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stat_fichier_disparu_nest_pas_une_erreur() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_stat_gone_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sous")).unwrap();
        std::fs::write(dir.join("sous/f.txt"), "x").unwrap();
        let root = dir.to_str().unwrap();

        std::fs::remove_file(dir.join("sous/f.txt")).unwrap();
        assert!(stat_project_file(root, "sous/f.txt").unwrap().is_none(), "fichier supprime = None");

        // Dossier parent supprime : le fichier a disparu aussi, toujours pas une panne.
        std::fs::remove_dir_all(dir.join("sous")).unwrap();
        assert!(stat_project_file(root, "sous/f.txt").unwrap().is_none(), "parent supprime = None");

        // Remplace par un dossier : ce n'est plus le fichier qu'on affichait.
        std::fs::create_dir_all(dir.join("d")).unwrap();
        assert!(stat_project_file(root, "d").unwrap().is_none(), "dossier = None");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_stat_refuse_de_sortir_de_la_racine() {
        let dir = std::env::temp_dir().join(format!("cockpit_ws_stat_esc_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let root = dir.to_str().unwrap();

        // Une remontee est refusee QUELLE QUE SOIT l'existence de la cible : c'est la regle
        // qu'on teste, pas la disposition du disque de la machine de test. La version
        // precedente passait sous Linux (« ../../etc » depuis /tmp tombe sur /etc, qui
        // existe) et echouait sur macOS, ou le dossier temporaire est profond et la cible
        // n'existe pas — d'ou une release macOS cassee.
        assert!(stat_project_file(root, "../../etc/passwd").is_err(), "traversee interdite");
        assert!(stat_project_file(root, "..").is_err(), "remontee simple interdite");
        assert!(
            stat_project_file(root, "../nulle_part_du_tout/x.txt").is_err(),
            "remontee vers une cible inexistante refusee aussi"
        );
        assert!(stat_project_file(root, "/etc/passwd").is_err(), "chemin absolu refuse");
        assert!(stat_project_file(root, "").is_err(), "chemin vide refuse");

        // Un chemin normal reste accepte : la garde ne doit pas tout interdire.
        std::fs::write(dir.join("ok.txt"), b"x").unwrap();
        assert!(stat_project_file(root, "ok.txt").unwrap().is_some(), "fichier normal lisible");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
