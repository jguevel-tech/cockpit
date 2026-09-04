//! Ce qui doit se passer quand la fenetre change d'ecran.
//!
//! **Le bug, avec son declencheur.** En passant d'un ecran a l'autre, la page reste dessinee a
//! son ANCIENNE taille : un rectangle de contenu en haut a gauche, du noir autour, et il faut
//! redemarrer. Constate le 2026-08-31 sur deux ecrans de facteur d'echelle different, session
//! Wayland avec l'application forcee en X11 — la ou le facteur d'echelle par ecran n'existe pas.
//!
//! GTK ne reallouе pas toujours le widget de la vue web quand ce facteur change. L'evenement,
//! lui, arrive bien : `ScaleFactorChanged` porte la nouvelle taille. Personne ne l'ecoutait.
//!
//! **Pourquoi une secousse d'un pixel.** Redonner a la fenetre la taille qu'elle a DEJA ne
//! change rien pour GTK, donc ne declenche aucune reallocation : il faut une taille differente,
//! puis la bonne. C'est laid, mais ca n'arrive qu'au changement d'ecran, et l'alternative est une
//! fenetre inutilisable.
//!
//! **Uniquement sur `ScaleFactorChanged`.** Faire ce travail sur `Resized`, qui tombe a chaque
//! pixel d'un redimensionnement a la souris, couterait cher pour rien — et risquerait de se
//! rappeler lui-meme.
//!
//! Ce module porte aussi la FERMETURE : c'est le dernier moment ou l'on peut photographier les
//! terminaux, et c'est ce qui les rend « comme on les a quittes » au prochain demarrage.

use std::time::Duration;

use tauri::{Manager, PhysicalSize, Runtime, WindowEvent};

/// Le temps laisse a GTK entre les deux tailles. Assez pour qu'une reallocation ait lieu, assez
/// court pour que personne ne voie autre chose qu'un scintillement.
const REPOS: Duration = Duration::from_millis(60);

/// La taille intermediaire de la secousse : un pixel de plus, sans jamais deborder.
///
/// En pixels PHYSIQUES, jamais logiques : une taille logique dependrait du facteur d'echelle,
/// celui-la meme qui vient de changer.
pub fn taille_secouee(taille: PhysicalSize<u32>) -> PhysicalSize<u32> {
    PhysicalSize::new(taille.width.saturating_add(1).max(1), taille.height.max(1))
}

/// A brancher sur le constructeur Tauri.
pub fn sur_evenement<R: Runtime>(fenetre: &tauri::Window<R>, evenement: &WindowEvent) {
    // La fenetre se ferme : derniere occasion de photographier les terminaux. C'est fait ICI
    // et SYNCHRONEMENT, avant que Tauri ne rende la main — un travail lance en tache de fond
    // serait tue avec le processus, et la photo ne servirait a rien.
    //
    // `CloseRequested` et non `Destroyed` : a `Destroyed`, l'etat de l'application est deja
    // en train de disparaitre. Le cout se paie une fois, quand on ferme.
    if matches!(evenement, WindowEvent::CloseRequested { .. }) {
        if let Some(etat) = fenetre.try_state::<crate::AppState>() {
            etat.terminals.enregistrer_les_ecrans(&etat.db, true);
        }
        return;
    }
    let WindowEvent::ScaleFactorChanged { new_inner_size, .. } = evenement else {
        return;
    };
    let taille = *new_inner_size;
    let fenetre = fenetre.clone();
    tauri::async_runtime::spawn(async move {
        // `set_size` est dispatche vers le fil principal par Tauri : rien a faire de plus ici.
        if fenetre.set_size(taille_secouee(taille)).is_err() {
            return;
        }
        tokio::time::sleep(REPOS).await;
        let _ = fenetre.set_size(taille);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_secousse_change_bien_la_taille() {
        let taille = PhysicalSize::new(1280, 800);
        let secouee = taille_secouee(taille);
        assert_ne!(secouee.width, taille.width, "sans changement, GTK ne realloue rien");
        assert_eq!(secouee.height, taille.height);
    }

    /// Une taille au maximum ne doit pas deborder : un debordement rendrait une fenetre de un
    /// pixel, ce qui est pire que le bug corrige.
    #[test]
    fn la_secousse_ne_deborde_pas() {
        let secouee = taille_secouee(PhysicalSize::new(u32::MAX, u32::MAX));
        assert_eq!(secouee.width, u32::MAX);
        assert_eq!(secouee.height, u32::MAX);
    }

    #[test]
    fn une_taille_nulle_ne_donne_pas_une_fenetre_nulle() {
        let secouee = taille_secouee(PhysicalSize::new(0, 0));
        assert!(secouee.width >= 1 && secouee.height >= 1);
    }
}
