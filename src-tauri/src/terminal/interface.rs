//! L'interface que Cockpit demande a un serveur de terminaux, et RIEN de plus.
//!
//! Elle est ecrite depuis les besoins du frontend (les commandes Tauri de `lib.rs`). Les
//! deux operations qui n'existaient que pour contourner tmux — `detacher` et
//! `ecran_alternatif` — ont disparu avec lui a l'etape C du chantier
//! (`docs/portabilite/plan-terminaux.md`) : elles n'avaient aucun appelant.
//!
//! Ce qui traverse encore l'interface, et pourquoi :
//! - `AppHandle` seulement dans `preparer` : la sortie remonte au webview par un evenement
//!   Tauri, et l'implementation garde le handle une fois pour toutes. Ni `creer` ni
//!   `attacher` n'en ont besoin.
//! - `&Database` : le NOM d'onglet et le PROJET vivent en SQLite, parce qu'eux doivent
//!   survivre au redemarrage de la machine — le service, non. Le rowid est donc la seule
//!   identite qui traverse un reboot, et c'est lui que le service recoit.

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

/// Ce qu'un geste de recherche a trouve. Traverse l'IPC vers `terminalSearch`.
///
/// Le serveur ne PEINT rien — il n'a pas d'ecran : il rend ou se trouve l'occurrence, et
/// c'est le terminal du frontend qui defile et surligne. `ligne` suit la convention de la
/// grille : 0 est la premiere ligne visible, les valeurs negatives remontent dans
/// l'historique.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultatRecherche {
    /// Nombre d'occurrences du motif.
    pub total: u32,
    /// Indice de l'occurrence courante, de la plus ancienne a la plus recente.
    pub index: Option<u32>,
    pub ligne: Option<i32>,
    pub colonne: Option<u16>,
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
    /// Deux choses, et deux seulement : mettre le serveur en route, et reconcilier ce
    /// qu'il tient avec ce que la base dit (lignes sans session, sessions sans ligne). Des
    /// qu'un etat survit a l'application, les deux divergent.
    ///
    /// C'est aussi le seul endroit ou l'implementation recoit un `AppHandle` : elle le
    /// garde pour emettre la sortie des terminaux.
    fn preparer(&self, app: &AppHandle, db: &Database);

    /// Ouvre un terminal et rend son identifiant.
    fn creer(&self, db: &Database, demande: Creation) -> Result<i64, String>;

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
    /// NE REND RIEN, et doit etre GRATUITE quand le terminal est deja branche : le frontend
    /// l'appelle a chaque retour sur un onglet, et re-brancher declencherait un redessin
    /// complet — clignotement et retour en bas de l'historique a chaque changement d'onglet.
    /// L'etat retrouve arrive par le meme canal que la suite (`terminal_output`), jamais par
    /// un retour de fonction : le « replay » de l'ancienne implementation etait ignore
    /// depuis le pool de xterm (course replay/live -> ecran dechire).
    fn attacher(&self, db: &Database, id: i64, taille: Taille) -> Result<(), String>;

    /// Renomme un terminal (libelle d'onglet). BESOIN REEL, purement metadonnee.
    fn renommer(&self, db: &Database, id: i64, nom: &str) -> Result<(), String>;

    /// Photographie les terminaux vivants et range les photos en base.
    ///
    /// C'est ce qui rend les terminaux « comme on les a quittes » au prochain demarrage : le
    /// shell, lui, ne survit pas a l'extinction du poste. Ne rend RIEN : c'est un filet, et
    /// un service deja parti signifie simplement qu'il n'y a plus rien a photographier.
    ///
    /// **A appeler aux moments qui comptent, jamais en continu** : une photo coute un
    /// aller-retour par terminal et jusqu'a un mega-octet ecrit en base. L'implementation
    /// refuse d'ailleurs de recommencer trop vite, sauf si `force` — la fermeture de la
    /// fenetre est le seul moment ou l'on ne peut pas remettre a plus tard.
    fn enregistrer_les_ecrans(&self, db: &Database, force: bool);

    /// Les terminaux d'un projet, ou tous si `projet` est `None`. BESOIN REEL.
    ///
    /// Porte le flag `llm` (un agent tourne dedans) : c'est le logo Claude de la barre
    /// laterale, et c'est appele toutes les 5 s. Une implementation qui rendrait cette
    /// reponse chere se paierait immediatement.
    fn lister(&self, db: &Database, projet: Option<&str>) -> Vec<TerminalInfo>;

    /// Cherche dans le terminal, historique compris. BESOIN REEL.
    ///
    /// Le motif est une SOUS-CHAINE LITTERALE, pas une regex : c'est une recherche
    /// d'utilisateur, « 1.2.3 » ne doit pas trouver « 1x2y3 ». Rend OU se trouve
    /// l'occurrence : le serveur n'a pas d'ecran a peindre, c'est le terminal du frontend
    /// qui defile et surligne.
    fn chercher(
        &self,
        db: &Database,
        id: i64,
        action: ActionRecherche,
        motif: &str,
    ) -> Result<ResultatRecherche, String>;
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
