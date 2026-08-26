//! Les conversations passees d'un projet, chez le fournisseur qui sait en garder.
//!
//! **Le fournisseur ne fait que LIRE les siennes.** Le nom qu'on leur donne a la main, lui,
//! vit dans notre base : c'est notre donnee, pas la sienne, et elle doit survivre a une
//! desinstallation de son CLI comme a un changement de format de sa part.
//!
//! Un identifiant de conversation n'a de sens que chez son fournisseur — un UUID pour l'un, un
//! numero pour l'autre — d'ou une cle a DEUX colonnes. Sans le fournisseur dans la cle, deux
//! conversations differentes portant le meme identifiant chez deux produits differents
//! echangeraient leurs noms.

use crate::storage::Database;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// Combien de conversations on rend, les plus recentes d'abord. Un menu deroulant n'a pas a
/// porter trois ans d'historique.
pub const MAX: usize = 15;

/// Une conversation telle que son fournisseur la connait.
pub struct ConversationBrute {
    pub id: String,
    /// De quoi la reconnaitre : le fournisseur en tire ce qu'il peut (premier message, titre).
    pub label: String,
    /// Epoch secondes de la derniere activite.
    pub updated_at: i64,
}

/// Une conversation telle qu'on l'affiche, nom personnalise applique.
#[derive(Serialize, Clone, Debug)]
pub struct Conversation {
    pub id: String,
    pub label: String,
    pub updated_at: i64,
    /// Le nom vient d'un renommage : l'interface le signale, et le renommer repart de la.
    pub renamed: bool,
}

/// Ce qu'un fournisseur doit savoir faire pour que le bouton du terminal existe.
pub trait Conversations: Send + Sync {
    /// Les conversations du projet, les plus recentes d'abord, au plus `max`.
    ///
    /// Un dossier absent n'est PAS une panne : ce projet n'a simplement pas encore de
    /// conversation. En revanche un dossier personnel introuvable en est une, et elle remonte
    /// — sinon la liste sort vide et on cherche la panne du cote du fournisseur.
    fn lister(&self, projet: &Path, max: usize) -> Result<Vec<ConversationBrute>, String>;

    /// La commande qui reprend une conversation dans un terminal neuf.
    fn commande_de_reprise(&self, id: &str) -> String;

    /// La commande qui en demarre une neuve.
    fn commande_neuve(&self) -> String;
}

/// Les conversations d'un projet, noms personnalises appliques.
pub fn lister(
    db: &Database,
    fournisseur: &'static dyn super::Fournisseur,
    projet: &str,
) -> Result<Vec<Conversation>, String> {
    let Some(lecteur) = fournisseur.conversations() else {
        // Le frontend cache le bouton dans ce cas : y arriver quand meme veut dire que
        // quelque chose a change entre l'affichage et le clic. On le DIT.
        return Err(format!(
            "{} ne sait pas retrouver ses conversations passees",
            fournisseur.nom()
        ));
    };
    let noms = noms_personnalises(db, fournisseur.id());
    Ok(lecteur
        .lister(Path::new(projet), MAX)?
        .into_iter()
        .map(|brute| match noms.get(&brute.id) {
            Some(nom) => Conversation {
                id: brute.id,
                label: nom.clone(),
                updated_at: brute.updated_at,
                renamed: true,
            },
            None => Conversation {
                id: brute.id,
                label: brute.label,
                updated_at: brute.updated_at,
                renamed: false,
            },
        })
        .collect())
}

/// Renomme une conversation. Un nom vide efface le renommage et rend son label automatique.
pub fn renommer(db: &Database, fournisseur: &str, id: &str, nom: &str) -> Result<(), String> {
    let propre = nom.trim();
    let conn = db.conn();
    if propre.is_empty() {
        conn.execute(
            "DELETE FROM noms_conversations WHERE fournisseur=?1 AND conversation_id=?2",
            rusqlite::params![fournisseur, id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO noms_conversations (fournisseur, conversation_id, nom) VALUES (?1, ?2, ?3)
             ON CONFLICT(fournisseur, conversation_id) DO UPDATE SET nom=excluded.nom",
            rusqlite::params![fournisseur, id, propre.chars().take(80).collect::<String>()],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Les noms poses a la main, pour ce fournisseur.
///
/// Une lecture qui echoue rend une table vide et non une erreur : on perdrait l'acces a toutes
/// les conversations pour un nom d'affichage.
pub(crate) fn noms_personnalises(db: &Database, fournisseur: &str) -> HashMap<String, String> {
    let conn = db.conn();
    let Ok(mut requete) = conn.prepare(
        "SELECT conversation_id, nom FROM noms_conversations WHERE fournisseur=?1",
    ) else {
        return HashMap::new();
    };
    let Ok(lignes) = requete.query_map([fournisseur], |l| {
        Ok((l.get::<_, String>(0)?, l.get::<_, String>(1)?))
    }) else {
        return HashMap::new();
    };
    lignes.flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_nom_pose_a_la_main_est_relu_puis_effacable() {
        let db = Database::new(":memory:").unwrap();
        renommer(&db, "claude", "abc-123", "audit du forum").unwrap();
        assert_eq!(noms_personnalises(&db, "claude").get("abc-123").unwrap(), "audit du forum");

        renommer(&db, "claude", "abc-123", "nouveau nom").unwrap();
        assert_eq!(noms_personnalises(&db, "claude").get("abc-123").unwrap(), "nouveau nom");

        renommer(&db, "claude", "abc-123", "  ").unwrap();
        assert!(noms_personnalises(&db, "claude").get("abc-123").is_none());
    }

    /// **LA CLE PORTE LE FOURNISSEUR.** Un identifiant de conversation n'a de sens que chez le
    /// sien : deux produits peuvent tres bien numeroter leurs conversations pareil, et sans
    /// cette colonne ils echangeraient leurs noms.
    #[test]
    fn deux_fournisseurs_ne_partagent_pas_leurs_noms() {
        let db = Database::new(":memory:").unwrap();
        renommer(&db, "claude", "1", "chez claude").unwrap();
        renommer(&db, "codex", "1", "chez codex").unwrap();

        assert_eq!(noms_personnalises(&db, "claude").get("1").unwrap(), "chez claude");
        assert_eq!(noms_personnalises(&db, "codex").get("1").unwrap(), "chez codex");
    }

    /// Demander ses conversations a un fournisseur qui n'en garde pas doit le DIRE. Rendre une
    /// liste vide fabriquerait un mensonge : « aucune conversation » au lieu de « je ne sais
    /// pas faire ».
    #[test]
    fn un_fournisseur_sans_conversations_le_dit() {
        let db = Database::new(":memory:").unwrap();
        let codex = super::super::par_id("codex").unwrap();
        let erreur = lister(&db, codex, "/tmp/projet").unwrap_err();
        assert!(erreur.contains("Codex"), "le message doit nommer le fournisseur : {erreur}");
    }
}
