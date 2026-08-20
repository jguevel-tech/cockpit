//! Lire l'ecran comme du TEXTE : chercher dedans, et en extraire une region.
//!
//! Les deux services que tmux rendait par son copy-mode, et que le service maison rend
//! sans lancer un seul process (`docs/portabilite/plan-terminaux.md`, etape B2).
//!
//! **Une ligne enroulee compte pour UNE.** Une commande longue collee dans un terminal de
//! 80 colonnes occupe trois rangees ; y chercher « --no-bundle » a cheval sur la coupure
//! doit trouver. C'est le fanion `WRAPLINE` de la derniere cellule d'une rangee qui dit
//! que la suivante la prolonge.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;

use super::Ecran;

/// Une occurrence trouvee dans l'ecran ou son historique.
///
/// `ligne` est l'indice d'alacritty : 0 est la premiere ligne VISIBLE, les valeurs
/// negatives remontent dans l'historique. Le frontend en deduit de combien defiler.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Occurrence {
    pub ligne: i32,
    pub colonne: u16,
    /// Longueur en CELLULES, qui peut deborder sur la rangee suivante si la ligne
    /// logique est enroulee.
    pub longueur: u16,
}

/// Une ligne logique : son texte, et la cellule d'ou vient chaque caractere.
struct LigneLogique {
    texte: Vec<char>,
    /// Meme longueur que `texte`. Un caractere combinant repete la position du precedent.
    ou: Vec<(i32, u16)>,
}

impl Ecran {
    /// Toutes les occurrences du motif, de la plus ancienne a la plus recente.
    ///
    /// Le motif est une SOUS-CHAINE LITTERALE, jamais une regex : c'est une recherche
    /// d'utilisateur, « 1.2.3 » ne doit pas trouver « 1x2y3 ». La casse est ignoree,
    /// comme dans la recherche de fichiers du projet.
    pub fn chercher(&self, motif: &str) -> Vec<Occurrence> {
        let motif: Vec<char> = motif.chars().map(minuscule).collect();
        if motif.is_empty() {
            return Vec::new();
        }
        let mut trouvees = Vec::new();
        // Une ligne logique a la fois : sa carte de positions est jetee des qu'elle est
        // fouillee. Garder celle des 10 000 lignes d'un coup couterait plusieurs Mo pour
        // une recherche qui n'en a besoin que d'une.
        self.pour_chaque_ligne_logique(|ligne| {
            for depart in 0..ligne.texte.len().saturating_sub(motif.len() - 1) {
                if ligne.texte[depart..depart + motif.len()] == motif[..] {
                    let (l, c) = ligne.ou[depart];
                    trouvees.push(Occurrence {
                        ligne: l,
                        colonne: c,
                        longueur: motif.len().min(u16::MAX as usize) as u16,
                    });
                }
            }
        });
        trouvees
    }

    /// Le texte compris entre deux cellules, bornes COMPRISES.
    ///
    /// Les rangees enroulees sont recollees sans saut de ligne : une commande de 200
    /// caracteres copiee depuis un terminal de 80 colonnes doit pouvoir etre recollee
    /// telle quelle dans un autre shell.
    pub fn texte_region(&self, debut: (i32, u16), fin: (i32, u16)) -> String {
        let grille = self.term().grid();
        let (debut, fin) = if debut <= fin { (debut, fin) } else { (fin, debut) };
        let premiere = debut.0.max(grille.topmost_line().0);
        let derniere = fin.0.min(grille.bottommost_line().0);
        let mut out = String::new();
        for l in premiere..=derniere {
            let depuis = if l == debut.0 { debut.1 as usize } else { 0 };
            let jusqu_a = if l == fin.0 { (fin.1 as usize + 1).min(self.colonnes()) } else { self.colonnes() };
            let mut rangee = String::new();
            for c in depuis..jusqu_a {
                let cellule = &grille[Line(l)][Column(c)];
                if cellule.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                rangee.push(cellule.c);
                for zero in cellule.zerowidth().unwrap_or(&[]) {
                    rangee.push(*zero);
                }
            }
            let enroulee = grille[Line(l)][Column(self.colonnes() - 1)]
                .flags
                .contains(Flags::WRAPLINE);
            if enroulee {
                out.push_str(&rangee);
            } else {
                // Une rangee est remplie d'espaces jusqu'au bout : les garder collerait
                // des dizaines de blancs a chaque fin de ligne copiee.
                out.push_str(rangee.trim_end());
                if l != derniere {
                    out.push('\n');
                }
            }
        }
        out
    }

    /// Parcourt l'historique puis l'ecran, une ligne LOGIQUE a la fois.
    fn pour_chaque_ligne_logique(&self, mut voir: impl FnMut(&LigneLogique)) {
        let grille = self.term().grid();
        let colonnes = self.colonnes();
        let mut courante = LigneLogique { texte: Vec::new(), ou: Vec::new() };
        for l in grille.topmost_line().0..=grille.bottommost_line().0 {
            for c in 0..colonnes {
                let cellule = &grille[Line(l)][Column(c)];
                if cellule.flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                    continue;
                }
                courante.texte.push(minuscule(cellule.c));
                courante.ou.push((l, c as u16));
                for zero in cellule.zerowidth().unwrap_or(&[]) {
                    courante.texte.push(minuscule(*zero));
                    courante.ou.push((l, c as u16));
                }
            }
            if grille[Line(l)][Column(colonnes - 1)].flags.contains(Flags::WRAPLINE) {
                continue; // la rangee suivante prolonge celle-ci
            }
            voir(&courante);
            courante.texte.clear();
            courante.ou.clear();
        }
        if !courante.texte.is_empty() {
            voir(&courante);
        }
    }
}

/// Minuscule d'un caractere, en gardant STRICTEMENT un caractere pour un.
///
/// `char::to_lowercase` peut en rendre plusieurs (« İ » turc en donne deux) : accepter ces
/// cas-la decalerait la carte des positions, et la recherche surlignerait a cote. Les
/// lettres accentuees usuelles (« É » -> « é ») en rendent bien un seul et sont donc
/// couvertes ; les rares expansions restent sensibles a la casse, ce qui est le pire cas
/// acceptable pour une recherche.
fn minuscule(c: char) -> char {
    let mut it = c.to_lowercase();
    match (it.next(), it.next()) {
        (Some(bas), None) => bas,
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::super::Ecran;

    fn ecran(octets: &str) -> Ecran {
        let mut e = Ecran::nouveau(20, 5);
        e.avaler(octets.as_bytes());
        e
    }

    #[test]
    fn trouve_une_sous_chaine_sur_la_ligne_visible() {
        let e = ecran("bonjour le monde");
        let trouvees = e.chercher("monde");
        assert_eq!(trouvees.len(), 1);
        assert_eq!((trouvees[0].ligne, trouvees[0].colonne, trouvees[0].longueur), (0, 11, 5));
    }

    #[test]
    fn la_casse_est_ignoree() {
        let e = ecran("Deploiement TERMINE");
        assert_eq!(e.chercher("termine").len(), 1);
        assert_eq!(e.chercher("DEPLOIEMENT").len(), 1);
    }

    /// La regle du trait : sous-chaine LITTERALE, pas une regex.
    #[test]
    fn le_motif_n_est_pas_une_regex() {
        let e = ecran("1x2y3");
        assert!(e.chercher("1.2.3").is_empty());
        assert_eq!(e.chercher("1x2").len(), 1);
    }

    /// Le cas qui justifie les lignes logiques : une commande trop longue pour la largeur.
    #[test]
    fn une_occurrence_a_cheval_sur_l_enroulement_est_trouvee() {
        let e = ecran("npx tauri build --no-bundle et la suite");
        let trouvees = e.chercher("--no-bundle");
        assert_eq!(trouvees.len(), 1, "{trouvees:?}");
        // Elle commence en colonne 16 de la premiere rangee (large de 20) et se termine
        // sur la seconde : sans le recollement des rangees enroulees, rien n'est trouve.
        assert_eq!((trouvees[0].ligne, trouvees[0].colonne, trouvees[0].longueur), (0, 16, 11));
    }

    #[test]
    fn les_occurrences_de_l_historique_portent_une_ligne_negative() {
        let mut e = Ecran::avec_historique(20, 3, 50);
        e.avaler(b"cible\r\n");
        for i in 0..10 {
            e.avaler(format!("bruit {i}\r\n").as_bytes());
        }
        let trouvees = e.chercher("cible");
        assert_eq!(trouvees.len(), 1);
        assert!(trouvees[0].ligne < 0, "ligne={}", trouvees[0].ligne);
    }

    #[test]
    fn un_motif_vide_ne_trouve_rien() {
        assert!(ecran("quoi que ce soit").chercher("").is_empty());
    }

    #[test]
    fn la_region_rend_le_texte_sans_les_blancs_de_remplissage() {
        let e = ecran("alpha\r\nbeta\r\ngamma");
        assert_eq!(e.texte_region((0, 0), (2, 19)), "alpha\nbeta\ngamma");
    }

    #[test]
    fn la_region_s_arrete_aux_colonnes_demandees() {
        let e = ecran("abcdefgh");
        assert_eq!(e.texte_region((0, 2), (0, 4)), "cde");
    }

    /// Une ligne enroulee se recolle : la commande copiee doit etre recollable telle quelle.
    #[test]
    fn la_region_recolle_les_rangees_enroulees() {
        let e = ecran("npx tauri build --no-bundle");
        assert_eq!(e.texte_region((0, 0), (1, 19)), "npx tauri build --no-bundle");
    }

    /// Les bornes a l'envers sont un glisser-deposer vers le haut, pas une erreur.
    #[test]
    fn les_bornes_inversees_donnent_la_meme_region() {
        let e = ecran("alpha\r\nbeta");
        assert_eq!(e.texte_region((1, 3), (0, 0)), e.texte_region((0, 0), (1, 3)));
    }
}
