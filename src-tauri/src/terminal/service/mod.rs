//! Notre propre service de terminaux : les shells vivent dans un processus a part, qui
//! survit a la fermeture de l'application.
//!
//! L'application lui parle par `terminal/adaptateur.rs`, qui implemente le trait
//! `Terminaux` par-dessus `client.rs`. Ce module se teste aussi seul (`tests.rs`) : il ne
//! connait ni Tauri, ni SQLite.
//!
//! | Fichier | Role |
//! |---|---|
//! | `protocole.rs` | ce qui circule, et la poignee de main versionnee |
//! | `tuyau.rs` | ou l'on ecoute, et comment on refuse un autre utilisateur |
//! | `session.rs` | un shell, son ecran, et ce qui remonte de sa sortie |
//! | `serveur.rs` | le service : accepte, distribue, tient les sessions |
//! | `client.rs` | le cote application de la conversation |
//! | `lancement.rs` | detacher le service pour qu'il survive a l'application |
//!
//! Trois choses tranchees une fois pour toutes, a ne pas rouvrir sans raison :
//! 1. **Le service n'ecrit rien sur disque.** Il meurt avec la machine, donc aucun format
//!    a migrer plus tard.
//! 2. **Un numero de version des la premiere version**, envoye par le SERVICE en premier,
//!    dans un preambule de forme figee.
//! 3. **Un service par utilisateur**, jamais un service systeme.

pub mod client;
pub mod lancement;
pub mod protocole;
pub mod serveur;
pub mod session;
pub mod tuyau;

#[cfg(test)]
mod tests;

use protocole::InfoSession;

/// Ce qu'il faut corriger pour que la base et le service disent la meme chose.
///
/// Des qu'un etat survit a l'application, les deux divergent : un shell qui se termine
/// pendant que Cockpit est ferme laisse une ligne sans session ; un service arrete
/// pendant que Cockpit tourne laisse l'inverse.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reconciliation {
    /// Terminaux presents en base dont le service n'a plus la session : leur shell est
    /// mort, la ligne doit disparaitre.
    pub lignes_a_supprimer: Vec<i64>,
    /// Sessions que le service tient et que la base ne connait pas : plus aucun onglet ne
    /// peut les afficher, leur shell tourne pour personne.
    pub sessions_orphelines: Vec<i64>,
}

/// Compare ce que le service tient et ce que la base dit.
///
/// Fonction PURE : ni base, ni socket. C'est l'etape C qui branchera les deux bouts, et
/// c'est ce qui permet de verrouiller la regle par un test au lieu d'une relecture.
///
/// Une session presente mais MORTE compte comme absente : son shell ne repondra plus.
pub fn reconcilier(sessions: &[InfoSession], lignes: &[i64]) -> Reconciliation {
    let vivantes: std::collections::HashSet<i64> =
        sessions.iter().filter(|s| s.vivant).map(|s| s.id).collect();
    let connues: std::collections::HashSet<i64> = lignes.iter().copied().collect();
    let mut resultat = Reconciliation {
        lignes_a_supprimer: lignes.iter().copied().filter(|id| !vivantes.contains(id)).collect(),
        sessions_orphelines: vivantes.iter().copied().filter(|id| !connues.contains(id)).collect(),
    };
    resultat.lignes_a_supprimer.sort_unstable();
    resultat.sessions_orphelines.sort_unstable();
    resultat
}

#[cfg(test)]
mod tests_reconciliation {
    use super::*;
    use protocole::Taille;

    fn session(id: i64, vivant: bool) -> InfoSession {
        InfoSession { id, vivant, llm: false, taille: Taille { colonnes: 80, lignes: 24 } }
    }

    #[test]
    fn rien_a_faire_quand_les_deux_sont_d_accord() {
        let vue = reconcilier(&[session(1, true), session(2, true)], &[1, 2]);
        assert_eq!(vue, Reconciliation::default());
    }

    /// Le cas du redemarrage de la machine : le service est parti avec, les lignes restent.
    #[test]
    fn les_lignes_sans_session_sont_a_supprimer() {
        let vue = reconcilier(&[], &[3, 1, 2]);
        assert_eq!(vue.lignes_a_supprimer, vec![1, 2, 3]);
        assert!(vue.sessions_orphelines.is_empty());
    }

    /// Une session que plus aucun onglet ne peut afficher : son shell tourne pour personne.
    #[test]
    fn les_sessions_sans_ligne_sont_orphelines() {
        let vue = reconcilier(&[session(9, true)], &[]);
        assert_eq!(vue.sessions_orphelines, vec![9]);
        assert!(vue.lignes_a_supprimer.is_empty());
    }

    /// Une session MORTE ne sauve pas sa ligne : le shell ne repondra plus.
    #[test]
    fn une_session_morte_compte_comme_absente() {
        let vue = reconcilier(&[session(4, false)], &[4]);
        assert_eq!(vue.lignes_a_supprimer, vec![4]);
        assert!(vue.sessions_orphelines.is_empty(), "{vue:?}");
    }
}
