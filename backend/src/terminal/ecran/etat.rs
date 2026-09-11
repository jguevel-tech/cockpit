//! La photo comparable d'un ecran.
//!
//! C'est le JUGE du test d'aller-retour : `EtatEcran` est ce qu'on compare entre l'ecran
//! d'origine et l'ecran obtenu en relisant les octets de redessin dans un emulateur neuf.
//! Une egalite, donc un test automatisable — c'est ce qui borne le travail de fidelite.
//!
//! Pourquoi ne pas comparer directement les `Term` d'alacritty :
//! - `Cell` porte un `extra: Option<Arc<CellExtra>>` dont l'absence et la presence-vide ne
//!   sont pas toujours interchangeables selon le chemin d'ecriture ; on met a plat.
//! - un `assert_eq!` sur deux grilles de 10 000 lignes ne dit RIEN de ce qui differe.
//!   `difference()` rend la PREMIERE divergence en une phrase, avec ses coordonnees.

use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::vte::ansi::{Color, CursorShape, Rgb, StandardCharset};

/// Une cellule mise a plat : tout ce qui la decrit, sans stockage partage.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Cellule {
    pub c: char,
    pub avant: Color,
    pub arriere: Color,
    pub fanions: Flags,
    /// Caracteres de largeur nulle poses sur cette cellule (accents combinants).
    pub combinants: Vec<char>,
    pub couleur_soulignement: Option<Color>,
    /// Lien OSC 8 : (identifiant, adresse).
    pub lien: Option<(String, String)>,
}

impl Cellule {
    pub fn depuis(cell: &Cell) -> Self {
        Self {
            c: cell.c,
            avant: cell.fg,
            arriere: cell.bg,
            fanions: cell.flags,
            combinants: cell.zerowidth().unwrap_or(&[]).to_vec(),
            couleur_soulignement: cell.underline_color(),
            lien: cell
                .hyperlink()
                .map(|lien| (lien.id().to_string(), lien.uri().to_string())),
        }
    }
}

/// Le curseur, et le « stylo » avec lequel la suite s'ecrira.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Curseur {
    /// Negative dans l'historique, 0 en haut de l'ecran visible.
    pub ligne: i32,
    pub colonne: usize,
    /// Le curseur est en butee a droite : le prochain caractere passe a la ligne AVANT
    /// d'etre ecrit. Un etat a part entiere, pas un detail — sans lui, le caractere
    /// suivant ecrase la derniere colonne au lieu de continuer sur la ligne d'apres.
    pub attend_retour: bool,
    /// Attributs courants (SGR). `c` est neutralise : le stylo n'a pas de caractere.
    pub stylo: Cellule,
    pub forme: CursorShape,
    pub clignote: bool,
}

/// Tout l'etat observable d'un ecran, mis a plat pour etre compare.
///
/// Ce qui n'y est PAS, et pourquoi :
/// - `display_offset` (position de la molette) : le redessin ramene toujours en bas de
///   l'historique, et c'est le comportement voulu — le defilement appartient a
///   l'interface, pas a l'etat du shell.
/// - les taquets de tabulation (HTS/TBC) et le curseur sauvegarde (DECSC) : ni l'un ni
///   l'autre n'est lisible depuis `Term`, donc ni comparable ni restaurable. Voir les cas
///   connus en tete de `mod.rs`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct EtatEcran {
    pub colonnes: usize,
    pub lignes: usize,
    /// L'ecran alternatif est actif (une application plein ecran tourne).
    pub alternatif: bool,
    /// Grille principale, de la plus vieille ligne d'historique a la derniere ligne visible.
    pub principale: Vec<Vec<Cellule>>,
    /// Grille alternative. Vide quand elle n'est pas active : alacritty la remet a zero a
    /// chaque entree en ecran alternatif, son contenu d'avant n'existe donc pas.
    pub alternative: Vec<Vec<Cellule>>,
    pub curseur: Curseur,
    /// Region de defilement (DECSTBM), bornes telles qu'alacritty les tient : debut
    /// inclus, fin exclue, en lignes de l'ecran visible.
    pub region: (i32, i32),
    /// Bits de `TermMode` (modes DEC).
    pub modes: u32,
    /// Entrees de palette modifiees par le programme (OSC 4/10/11/12).
    pub palette: Vec<(usize, Rgb)>,
    pub titre: Option<String>,
    pub pile_titres: Vec<Option<String>>,
    /// Index du jeu de caracteres actif (0 = G0).
    pub jeu_actif: usize,
    pub jeux: [StandardCharset; 4],
}

impl EtatEcran {
    /// La PREMIERE divergence avec `autre`, en une phrase lisible — ou `None` si les deux
    /// etats sont identiques.
    ///
    /// L'ordre des comparaisons va du plus structurant au plus fin : une taille qui ne
    /// colle pas rend inutile de parler d'une cellule.
    pub fn difference(&self, autre: &Self) -> Option<String> {
        if (self.colonnes, self.lignes) != (autre.colonnes, autre.lignes) {
            return Some(format!(
                "taille {}x{} contre {}x{}",
                self.colonnes, self.lignes, autre.colonnes, autre.lignes
            ));
        }
        if self.alternatif != autre.alternatif {
            return Some(format!(
                "ecran alternatif {} contre {}",
                self.alternatif, autre.alternatif
            ));
        }
        if let Some(quoi) = difference_grille("principale", &self.principale, &autre.principale) {
            return Some(quoi);
        }
        if let Some(quoi) = difference_grille("alternative", &self.alternative, &autre.alternative) {
            return Some(quoi);
        }
        if self.curseur != autre.curseur {
            return Some(format!(
                "curseur {:?}\n     contre {:?}",
                self.curseur, autre.curseur
            ));
        }
        if self.region != autre.region {
            return Some(format!(
                "region de defilement {:?} contre {:?}",
                self.region, autre.region
            ));
        }
        if self.modes != autre.modes {
            let ecart = self.modes ^ autre.modes;
            return Some(format!(
                "modes DEC {:#x} contre {:#x} (bits differents : {:#x})",
                self.modes, autre.modes, ecart
            ));
        }
        if self.palette != autre.palette {
            return Some(format!(
                "palette {:?} contre {:?}",
                self.palette, autre.palette
            ));
        }
        if self.titre != autre.titre {
            return Some(format!("titre {:?} contre {:?}", self.titre, autre.titre));
        }
        if self.pile_titres != autre.pile_titres {
            return Some(format!(
                "pile de titres {:?} contre {:?}",
                self.pile_titres, autre.pile_titres
            ));
        }
        if (self.jeu_actif, self.jeux) != (autre.jeu_actif, autre.jeux) {
            return Some(format!(
                "jeux de caracteres {} {:?} contre {} {:?}",
                self.jeu_actif, self.jeux, autre.jeu_actif, autre.jeux
            ));
        }
        None
    }
}

fn difference_grille(
    nom: &str,
    gauche: &[Vec<Cellule>],
    droite: &[Vec<Cellule>],
) -> Option<String> {
    if gauche.len() != droite.len() {
        return Some(format!(
            "grille {nom} : {} lignes contre {} (historique different)",
            gauche.len(),
            droite.len()
        ));
    }
    for (i, (lg, ld)) in gauche.iter().zip(droite).enumerate() {
        for (j, (cg, cd)) in lg.iter().zip(ld).enumerate() {
            if cg != cd {
                let depart = j.saturating_sub(2);
                let arrivee = (j + 3).min(lg.len());
                return Some(format!(
                    "grille {nom}, ligne {i} colonne {j} :\n     {cg:?}\n     contre {cd:?}\n\
                     \n     ligne attendue : {:?}\n     ligne obtenue  : {:?}\n\
                     \n     voisinage attendu : {:?}\n     voisinage obtenu  : {:?}",
                    texte_de_ligne(lg),
                    texte_de_ligne(ld),
                    resume_cellules(&lg[depart..arrivee]),
                    resume_cellules(&ld[depart..arrivee]),
                ));
            }
        }
    }
    None
}

/// Les cellules autour d'une divergence, en court : caractere et fanions. Sans ce
/// voisinage, une divergence de fanion de caractere large est indechiffrable — c'est la
/// cellule d'a cote qui explique celle qui differe.
fn resume_cellules(cellules: &[Cellule]) -> Vec<String> {
    cellules
        .iter()
        .map(|c| format!("{:?}/{:?}", c.c, c.fanions))
        .collect()
}

/// Le texte d'une ligne, pour situer une divergence de cellule dans son contexte.
fn texte_de_ligne(ligne: &[Cellule]) -> String {
    let mut s: String = ligne.iter().map(|c| c.c).collect();
    while s.ends_with(' ') {
        s.pop();
    }
    s
}
