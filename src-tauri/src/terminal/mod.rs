//! Terminaux integres persistants.
//!
//! `interface` decrit ce que Cockpit demande a un serveur de terminaux (trait `Terminaux`).
//! `tmux` est l'implementation actuelle, derriere ce trait. Le choix se fait ICI, en un
//! endroit : `terminaux()`.
//!
//! Chantier en cours — remplacer tmux par notre propre service, le meme sur les trois
//! systemes : `docs/portabilite/plan-terminaux.md`.

pub mod ecran;
pub mod history;
mod interface;
mod tmux;

pub use interface::{ActionRecherche, Creation, Taille, TerminalInfo, Terminaux};

/// L'implementation utilisee par l'application. Une seule ligne a changer le jour de la
/// bascule (etape C du chantier).
pub fn terminaux() -> Box<dyn Terminaux> {
    Box::new(tmux::TerminauxTmux::default())
}
