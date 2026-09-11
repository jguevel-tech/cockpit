//! Terminaux integres persistants.
//!
//! `interface` decrit ce que Cockpit demande a un serveur de terminaux (trait `Terminaux`),
//! `service` EST ce serveur (un processus a nous, qui survit a la fermeture de
//! l'application), et `adaptateur` fait le lien entre les deux. Le choix de
//! l'implementation se fait ICI, en un endroit : `terminaux()`.
//!
//! Historique : jusqu'a la v0.38 les terminaux etaient des sessions tmux. Le chantier qui
//! a remplace tmux par notre propre service — le meme sur les trois systemes — est decrit
//! dans `docs/portabilite/plan-terminaux.md`.

pub mod adaptateur;
pub mod agents_llm;
pub mod ecran;
pub mod environnement;
pub mod history;
mod interface;
pub mod service;

pub use interface::{
    ActionRecherche, Creation, ResultatRecherche, Taille, TerminalInfo, Terminaux,
};

/// L'implementation utilisee par l'application.
pub fn terminaux() -> Box<dyn Terminaux> {
    Box::new(adaptateur::TerminauxService::default())
}
