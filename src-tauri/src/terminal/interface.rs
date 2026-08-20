//! L'interface que Cockpit demande a un serveur de terminaux, et RIEN de plus.
//!
//! Elle est ecrite depuis les besoins du frontend (les commandes Tauri de `lib.rs`), pas
//! depuis ce que tmux sait faire : c'est tout l'objet de l'etape A du chantier
//! (`docs/portabilite/plan-terminaux.md`). Chaque operation porte ci-dessous si elle est un
//! BESOIN de Cockpit ou un CONTOURNEMENT de tmux — les seconds sont a ne pas reproduire
//! dans le service maison.
//!
//! Deux dependances traversent l'interface et ne sont pas des details :
//! - `AppHandle` : la sortie d'un terminal remonte au webview par un evenement Tauri. Toute
//!   implementation qui attache doit pouvoir emettre. Besoin reel.
//! - `&Database` : aujourd'hui l'identite d'un terminal (`id` -> nom de session tmux) vit en
//!   SQLite, donc presque chaque operation relit la base pour savoir a QUI parler. Un service
//!   qui tient ses propres metadonnees n'en aurait besoin que pour lister — c'est le point a
//!   trancher a l'etape B, pas ici.

use crate::storage::Database;
use serde::Serialize;
use tauri::AppHandle;

/// Ce que le frontend affiche d'un terminal. Forme figee : elle traverse l'IPC en
/// snake_case vers `src/lib/types/index.ts`.
#[derive(Serialize, Clone)]
pub struct TerminalInfo {
    pub id: i64,
    pub project: String,
    pub name: String,
    pub alive: bool,
    /// Un CLI d'agent LLM (claude, codex, gemini...) tourne dans la session.
    pub llm: bool,
}

/// Taille de la grille, en cellules. Nommee pour elle-meme parce que l'ordre
/// (colonnes, lignes) et (lignes, colonnes) s'inverse d'une API a l'autre — `PtySize` de
/// portable-pty prend les lignes d'abord, nos commandes Tauri les colonnes.
#[derive(Clone, Copy)]
pub struct Taille {
    pub colonnes: u16,
    pub lignes: u16,
}

/// Tout ce qu'il faut pour ouvrir un terminal.
pub struct Creation {
    /// Projet auquel le terminal appartient (cle des metadonnees, pas un chemin).
    pub projet: String,
    /// Dossier de depart du shell. Ignore s'il n'existe pas.
    pub dossier: String,
    pub taille: Taille,
    /// Commande a lancer des l'ouverture (bouton « ▶ Cmd », palette, shell de conteneur).
    pub commande_initiale: Option<String>,
}

/// Les quatre gestes de la recherche dans un terminal, tels que la barre d'onglets les
/// declenche. Un type plutot qu'une chaine : le frontend n'envoie que ces quatre valeurs
/// (union TypeScript de `terminalSearch`), et une faute de frappe doit se voir a la
/// compilation, pas dans un `match` a l'execution.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionRecherche {
    /// Chercher le motif vers le haut (entre dans l'historique si besoin).
    Demarrer,
    /// Occurrence plus ancienne.
    Suivante,
    /// Occurrence plus recente.
    Precedente,
    /// Sortir de la recherche.
    Annuler,
}

impl ActionRecherche {
    /// Traduit ce que la commande Tauri recoit. L'inconnu est une ERREUR remontee, pas un
    /// silence : une action qui ne fait rien serait vecue comme un bouton casse.
    pub fn depuis_texte(action: &str) -> Result<Self, String> {
        match action {
            "start" => Ok(Self::Demarrer),
            "next" => Ok(Self::Suivante),
            "prev" => Ok(Self::Precedente),
            "cancel" => Ok(Self::Annuler),
            autre => Err(format!("action de recherche inconnue: {}", autre)),
        }
    }
}

/// Un serveur de terminaux persistants, vu par Cockpit.
///
/// `Send + Sync` : l'implementation vit dans `AppState`, partagee par toutes les commandes.
pub trait Terminaux: Send + Sync {
    /// Appelee UNE FOIS au demarrage de l'application, avant toute autre operation.
    ///
    /// BESOIN REEL, mais un seul : reconcilier ce que le serveur tient et ce que la base
    /// dit (lignes sans session, sessions sans ligne). Des qu'un etat survit a l'app, les
    /// deux divergent. Tout le reste de ce que l'implementation tmux fait ici — deployer un
    /// binaire, reposer 41 options sur un serveur deja vivant — est un CONTOURNEMENT qui
    /// disparaitra avec elle.
    fn preparer(&self, app: &AppHandle, db: &Database);

    /// Ouvre un terminal et rend son identifiant.
    fn creer(&self, app: AppHandle, db: &Database, demande: Creation) -> Result<i64, String>;

    /// Envoie de la frappe au shell. BESOIN REEL, et le chemin le plus chaud de tout le
    /// projet : aucune allocation superflue, aucun fork, aucune lecture de base ici.
    fn ecrire(&self, id: i64, donnees: &str) -> Result<(), String>;

    /// Aligne le terminal sur la taille de son conteneur.
    fn redimensionner(&self, db: &Database, id: i64, taille: Taille) -> Result<(), String>;

    /// Ferme le terminal : le shell meurt et la ligne disparait.
    fn fermer(&self, db: &Database, id: i64) -> Result<(), String>;

    /// Branche l'interface sur un terminal existant : a partir de la, sa sortie remonte par
    /// l'evenement `terminal_output`.
    ///
    /// NE REND RIEN. L'implementation tmux rendait un « replay » que le frontend ignorait
    /// depuis le passage au pool de xterm (course replay/live -> ecran dechire) ; la notion
    /// n'a donc pas sa place dans l'interface. Un service qui redessine l'ecran au retour le
    /// fera par le meme canal que le reste, pas par un retour de fonction.
    fn attacher(&self, app: AppHandle, db: &Database, id: i64, taille: Taille) -> Result<(), String>;

    /// Cesse de remonter la sortie de ce terminal (le shell continue de tourner).
    ///
    /// CONTOURNEMENT — et meme du code mort aujourd'hui : depuis la doctrine du pool
    /// persistant (2026-08-13) le frontend n'appelle jamais `detach_terminal`, les xterm
    /// vivent dans un pool au niveau module et les ecouteurs sont globaux. Conserve parce que
    /// la commande Tauri existe encore ; a supprimer avec elle, pas a reimplementer.
    fn detacher(&self, id: i64);

    /// Renomme un terminal (libelle d'onglet). BESOIN REEL, purement metadonnee.
    fn renommer(&self, db: &Database, id: i64, nom: &str) -> Result<(), String>;

    /// Les terminaux d'un projet, ou tous si `projet` est `None`. BESOIN REEL.
    ///
    /// Porte le flag `llm` (un agent tourne dedans) : c'est le logo Claude de la barre
    /// laterale, et c'est appele toutes les 5 s. Une implementation qui rendrait cette
    /// reponse chere se paierait immediatement.
    fn lister(&self, db: &Database, projet: Option<&str>) -> Vec<TerminalInfo>;

    /// Le programme qui tourne dans le terminal occupe-t-il tout l'ecran (vim, claude...) ?
    ///
    /// CONTOURNEMENT PUR, et mort lui aussi : le client tmux met TOUJOURS le terminal hote en
    /// ecran alternatif, donc xterm ne peut pas repondre et il faut poser la question a tmux
    /// (un fork+exec par question). Un service maison tient la reponse en memoire, dans la
    /// grille — cette operation ne doit pas etre reimplementee comme une question distante.
    /// Aucun appelant cote frontend depuis que la molette est laissee au copy-mode.
    fn ecran_alternatif(&self, db: &Database, id: i64) -> bool;

    /// Cherche dans le terminal, historique compris. BESOIN REEL.
    ///
    /// Le motif est une SOUS-CHAINE LITTERALE, pas une regex : c'est une recherche
    /// d'utilisateur, « 1.2.3 » ne doit pas trouver « 1x2y3 ».
    fn chercher(
        &self,
        db: &Database,
        id: i64,
        action: ActionRecherche,
        motif: &str,
    ) -> Result<(), String>;

    /// Copie la selection courante vers le presse-papier systeme. BESOIN REEL (clic droit >
    /// Copier), mais aujourd'hui paye en cinq maillons parce que la selection appartient a
    /// tmux : le service maison la possede, ce sera un appel.
    fn copier_selection(&self, db: &Database, id: i64) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::ActionRecherche;

    /// Verrouille le contrat IPC : ces quatre mots sont exactement ce que
    /// `terminalSearch` (src/lib/api/workspace.ts) sait envoyer. En renommer un ici casse
    /// la loupe de la barre d'onglets en silence.
    #[test]
    fn les_quatre_actions_du_frontend_sont_reconnues() {
        assert_eq!(ActionRecherche::depuis_texte("start"), Ok(ActionRecherche::Demarrer));
        assert_eq!(ActionRecherche::depuis_texte("next"), Ok(ActionRecherche::Suivante));
        assert_eq!(ActionRecherche::depuis_texte("prev"), Ok(ActionRecherche::Precedente));
        assert_eq!(ActionRecherche::depuis_texte("cancel"), Ok(ActionRecherche::Annuler));
    }

    /// Le reste est une erreur REMONTEE, pas un silence : un bouton qui ne fait rien est
    /// vecu comme une panne.
    #[test]
    fn une_action_inconnue_est_une_erreur_explicite() {
        let err = ActionRecherche::depuis_texte("Start").unwrap_err();
        assert!(err.contains("Start"), "{err}");
        assert!(ActionRecherche::depuis_texte("").is_err());
    }
}
