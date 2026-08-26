//! Claude Code : ses conversations passees, et l'etat de son abonnement.
//!
//! **Tout ce qui est ici connait des chemins et des formats qui ne sont pas les notres.** Le
//! dossier `~/.claude`, l'encodage du chemin d'un projet, la forme des fichiers `.jsonl`, le
//! bloc `claudeAiOauth` du fichier de jetons : c'est la configuration d'un AUTRE logiciel, et
//! elle peut changer sans nous prevenir. C'est justement la raison d'etre du catalogue — cette
//! connaissance est enfermee dans ce fichier, et rien ailleurs n'en depend.

use crate::llm::abonnement::{self, Abonnement, ConnexionGuidee, Etat};
use crate::llm::conversations::{ConversationBrute, Conversations};
use crate::llm::Fournisseur;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

/// On ne lit que le debut d'un fichier de conversation pour en tirer un libelle.
const OCTETS_LUS: u64 = 256 * 1024;
const LIGNES_LUES: usize = 300;
/// Longueur d'un libelle tire du premier message.
const LONGUEUR_LIBELLE: usize = 90;

pub static CLAUDE: ClaudeCode = ClaudeCode;
static CONVERSATIONS: ConversationsClaude = ConversationsClaude;
static ABONNEMENT: AbonnementClaude = AbonnementClaude;

pub struct ClaudeCode;

impl Fournisseur for ClaudeCode {
    fn id(&self) -> &'static str {
        "claude"
    }
    fn nom(&self) -> &'static str {
        "Claude Code"
    }
    fn commandes(&self) -> &'static [&'static str] {
        &["claude"]
    }
    fn symbole(&self) -> &'static str {
        "✳"
    }
    fn couleur(&self) -> &'static str {
        "#d97757"
    }
    fn conversations(&self) -> Option<&'static dyn Conversations> {
        Some(&CONVERSATIONS)
    }
    fn abonnement(&self) -> Option<&'static dyn Abonnement> {
        Some(&ABONNEMENT)
    }
    fn plugins_claude_code(&self) -> bool {
        true
    }
}

// ---------- Les conversations passees ----------

pub struct ConversationsClaude;

impl Conversations for ConversationsClaude {
    fn lister(&self, projet: &Path, max: usize) -> Result<Vec<ConversationBrute>, String> {
        // Un dossier personnel introuvable REMONTE : sans ca la liste sortait vide et on
        // cherchait la panne du cote de Claude Code.
        let dossier = dossier_des_conversations(&projet.to_string_lossy())?;
        // Le dossier absent, lui, n'est pas une panne : ce projet n'a pas encore de
        // conversation.
        if !dossier.is_dir() {
            return Ok(Vec::new());
        }

        let mut trouvees: Vec<(i64, PathBuf, String)> = Vec::new();
        for entree in std::fs::read_dir(&dossier).map_err(|e| e.to_string())? {
            let Ok(entree) = entree else { continue };
            let chemin = entree.path();
            if chemin.extension().map(|e| e != "jsonl").unwrap_or(true) {
                continue;
            }
            let Some(id) = chemin.file_stem().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            let modifie = entree
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            trouvees.push((modifie, chemin, id));
        }

        // Les plus recentes d'abord, et on ne lit le libelle que de celles qu'on garde.
        trouvees.sort_by(|a, b| b.0.cmp(&a.0));
        trouvees.truncate(max);

        Ok(trouvees
            .into_iter()
            .map(|(modifie, chemin, id)| ConversationBrute {
                id,
                label: libelle(&chemin).unwrap_or_else(|| "(conversation)".into()),
                updated_at: modifie,
            })
            .collect())
    }

    fn commande_de_reprise(&self, id: &str) -> String {
        format!("claude --resume {id}")
    }

    fn commande_neuve(&self) -> String {
        "claude".to_string()
    }
}

/// L'encodage que Claude Code applique au chemin d'un projet : tout ce qui n'est pas
/// alphanumerique devient un tiret.
fn encoder_le_chemin(chemin: &str) -> String {
    chemin
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn dossier_des_conversations(projet: &str) -> Result<PathBuf, String> {
    Ok(crate::chemins::dossier_personnel()?
        .join(".claude/projects")
        .join(encoder_le_chemin(projet)))
}

/// Le premier message VRAIMENT humain du fichier, tronque pour servir de libelle.
///
/// Hors sidechains (les conversations d'agents lances par l'agent) et hors contenus injectes
/// par le harnais, qui commencent par une balise ou par « Caveat: » : les prendre donnait des
/// libelles tous identiques, tires d'un rappel systeme.
fn libelle(chemin: &Path) -> Option<String> {
    let fichier = std::fs::File::open(chemin).ok()?;
    let lecteur = BufReader::new(fichier.take(OCTETS_LUS));

    for ligne in lecteur.lines().take(LIGNES_LUES) {
        let Ok(ligne) = ligne else { break };
        let Ok(valeur) = serde_json::from_str::<serde_json::Value>(&ligne) else { continue };
        if valeur.get("type").and_then(|t| t.as_str()) != Some("user") {
            continue;
        }
        if valeur.get("isSidechain").and_then(|b| b.as_bool()) == Some(true) {
            continue;
        }
        let Some(texte) = texte_du_message(&valeur) else { continue };
        let texte = texte.trim();
        if texte.is_empty() || texte.starts_with('<') || texte.starts_with("Caveat:") {
            continue;
        }
        let coupe: String = texte.chars().take(LONGUEUR_LIBELLE).collect();
        return Some(if texte.chars().count() > LONGUEUR_LIBELLE {
            coupe + "…"
        } else {
            coupe
        });
    }
    None
}

fn texte_du_message(valeur: &serde_json::Value) -> Option<String> {
    let contenu = valeur.get("message")?.get("content")?;
    if let Some(texte) = contenu.as_str() {
        return Some(texte.to_string());
    }
    for morceau in contenu.as_array()? {
        if morceau.get("type").and_then(|t| t.as_str()) == Some("text") {
            return morceau.get("text").and_then(|t| t.as_str()).map(String::from);
        }
    }
    None
}

// ---------- L'abonnement ----------

pub struct AbonnementClaude;

impl Abonnement for AbonnementClaude {
    fn etat(&self) -> Etat {
        let mut etat = Etat::default();

        let chemin = match crate::chemins::dossier_personnel() {
            Ok(maison) => maison.join(".claude").join(".credentials.json"),
            Err(e) => {
                etat.probleme = Some(e);
                return etat;
            }
        };
        // Fichier absent = pas encore connecte. C'est le cas normal, pas un probleme a
        // signaler.
        let brut = match std::fs::read_to_string(&chemin) {
            Ok(brut) => brut,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return etat,
            Err(e) => {
                etat.probleme = Some(format!("{} illisible : {e}", chemin.display()));
                return etat;
            }
        };
        let json = match serde_json::from_str::<serde_json::Value>(&brut) {
            Ok(json) => json,
            Err(e) => {
                etat.probleme = Some(format!("{} n'est pas du JSON valide : {e}", chemin.display()));
                return etat;
            }
        };
        let Some(oauth) = json.get("claudeAiOauth") else {
            etat.probleme =
                Some(format!("{} ne contient pas de bloc claudeAiOauth", chemin.display()));
            return etat;
        };

        etat.connecte = oauth
            .get("accessToken")
            .and_then(|t| t.as_str())
            .map(|t| !t.is_empty())
            .unwrap_or(false);
        etat.formule = oauth.get("subscriptionType").and_then(|v| v.as_str()).map(String::from);
        etat.palier = oauth.get("rateLimitTier").and_then(|v| v.as_str()).map(String::from);
        etat.expire_le = oauth.get("expiresAt").and_then(|v| v.as_i64()).map(|ts| {
            // Des millisecondes quand la valeur est trop grande pour des secondes.
            if ts > 100_000_000_000 {
                ts / 1000
            } else {
                ts
            }
        });
        etat
    }

    fn connexion_guidee(&self) -> Option<ConnexionGuidee> {
        Some(ConnexionGuidee { programme: "claude", arguments: &["setup-token"] })
    }

    fn version_cli(&self) -> Option<String> {
        abonnement::version_par_cli("claude")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_encodage_du_chemin_suit_celui_de_claude_code() {
        assert_eq!(
            encoder_le_chemin("/home/jguevel/Documents/workspace/core/cockpit"),
            "-home-jguevel-Documents-workspace-core-cockpit"
        );
        assert_eq!(encoder_le_chemin("/a/b.c_d"), "-a-b-c-d");
    }

    #[test]
    fn le_libelle_saute_ce_qui_n_est_pas_un_vrai_message() {
        let dossier =
            std::env::temp_dir().join(format!("cockpit_llm_claude_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dossier);
        std::fs::create_dir_all(&dossier).unwrap();
        let fichier = dossier.join("abc.jsonl");
        std::fs::write(
            &fichier,
            concat!(
                "{\"type\":\"mode\",\"mode\":\"x\"}\n",
                "{\"type\":\"user\",\"isSidechain\":true,\"message\":{\"content\":\"sidechain a ignorer\"}}\n",
                "{\"type\":\"user\",\"message\":{\"content\":\"<system-reminder>injecte</system-reminder>\"}}\n",
                "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"corrige le bug du login\"}]}}\n",
            ),
        )
        .unwrap();

        assert_eq!(libelle(&fichier).unwrap(), "corrige le bug du login");
        let _ = std::fs::remove_dir_all(&dossier);
    }

    /// Les commandes que le bouton du terminal lance. Elles appartiennent au fournisseur : une
    /// autre marque ne reprend pas une conversation avec `--resume`.
    #[test]
    fn les_commandes_du_terminal_viennent_du_fournisseur() {
        assert_eq!(CONVERSATIONS.commande_neuve(), "claude");
        assert_eq!(CONVERSATIONS.commande_de_reprise("abc-123"), "claude --resume abc-123");
    }

    /// Un dossier de projet sans conversation n'est pas une panne : la liste est vide et
    /// personne ne cherche pourquoi.
    #[test]
    fn un_projet_sans_conversation_rend_une_liste_vide() {
        let liste = CONVERSATIONS
            .lister(Path::new("/chemin/qui/n/existe/pas/du/tout"), 15)
            .expect("un dossier absent n'est pas une erreur");
        assert!(liste.is_empty());
    }
}
