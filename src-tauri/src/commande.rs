//! Lancer un programme externe sans faire clignoter une console sous Windows.
//!
//! Une application GRAPHIQUE Windows n'a pas de console. Chaque programme console qu'elle
//! lance en ouvre donc une, le temps de son execution : une fenetre noire qui apparait et
//! disparait. Ce n'est pas un defaut cosmetique isole — le monitor Docker lance un
//! `docker compose ps` PAR PROJET toutes les cinq secondes, en permanence, plus le statut
//! git, la liste des conteneurs, la verification des URLs. Avec cinq projets, cinq
//! clignotements toutes les cinq secondes pendant toute la journee de travail.
//!
//! `CREATE_NO_WINDOW` supprime la console sans rien changer d'autre : les flux restent
//! redirigeables, le code de sortie et la sortie standard sont identiques. Sur Unix
//! l'implementation ne fait RIEN — c'est ce qui permet d'appeler `.sans_console()` partout
//! sans `#[cfg]` chez l'appelant.
//!
//! Aucun test automatique ne peut voir une fenetre clignoter : la seule protection est que
//! tout lancement passe par ici. Un `Command::new(...)` sans `.sans_console()` est un oubli.

/// <https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags>
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A appeler sur toute commande externe, juste apres `new()`.
pub trait SansConsole {
    fn sans_console(&mut self) -> &mut Self;
}

impl SansConsole for std::process::Command {
    fn sans_console(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

impl SansConsole for tokio::process::Command {
    fn sans_console(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            self.creation_flags(CREATE_NO_WINDOW);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le contrat est le meme sur les deux familles de commande, et l'appel doit rester
    /// chainable : c'est ce qui fait qu'on peut l'inserer dans un `Command::new().args()`
    /// existant sans le reecrire.
    #[test]
    fn l_appel_est_chainable_sur_les_deux_familles() {
        let mut std_cmd = std::process::Command::new("true");
        let programme = std_cmd.sans_console().arg("-n").get_program().to_owned();
        assert_eq!(programme, "true");

        let mut tokio_cmd = tokio::process::Command::new("true");
        tokio_cmd.sans_console().arg("-n");
        assert_eq!(tokio_cmd.as_std().get_program(), "true");
    }
}
