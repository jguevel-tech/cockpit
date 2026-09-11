//! L'ecran d'un terminal : la grille de caracteres, et les octets qui la redessinent.
//!
//! Deux services, et rien d'autre — ce module ne connait ni PTY, ni socket, ni Tauri :
//! 1. `avaler()` donne les octets du shell a l'emulateur et tient l'etat a jour ;
//! 2. `redessiner()` rend une suite d'octets qui refabrique cet etat dans un terminal NEUF.
//!
//! C'est le morceau ou est tout le risque du chantier des terminaux
//! (`docs/portabilite/plan-terminaux.md`, etape B1) : le test d'aller-retour de `tests.rs`
//! est ce qui le borne. Serialiser, relire dans un emulateur neuf, comparer les deux etats
//! cellule par cellule. Une egalite, donc un test.
//!
//! ## Ce sur quoi on s'appuie dans `alacritty_terminal` 0.26
//!
//! La crate ne promet AUCUNE stabilite d'API (0.x, extraite d'Alacritty), la version est
//! donc epinglee a l'exact dans `Cargo.toml`. Ce qui doit etre relu a chaque montee :
//! - `Term::{new, grid, grid_mut, resize, mode, colors, cursor_style, swap_alt}` ;
//! - `Grid::{cursor, display_offset, topmost_line, bottommost_line, total_lines}` et
//!   l'indexation `grid[Line][Column]` ;
//! - `Cell::{c, fg, bg, flags, zerowidth, underline_color, hyperlink}` et `Flags` ;
//! - `vte::ansi::{Processor, Handler}` (l'analyseur, et le gestionnaire qu'on espionne) ;
//! - `term::color::COUNT` (taille de la palette).
//!
//! ## Ce que `Term` garde prive, et comment on le recupere
//!
//! Trois etats necessaires au redessin ne sont pas lisibles : la region de defilement
//! (DECSTBM), le titre et sa pile, le jeu de caracteres actif. On les suit avec un
//! ESPION : le meme flux d'octets est donne a un SECOND analyseur dont le gestionnaire
//! n'implemente que ces quatre operations et ignore tout le reste.
//!
//! Pourquoi un second analyseur plutot qu'un gestionnaire qui envelopperait `Term` :
//! `Handler` compte 85 methodes a valeur par defaut vide. Un enveloppeur doit toutes les
//! reexpedier, et une faute de frappe dans l'une d'elles (`move_up` renvoye vers
//! `move_down`) casserait l'EMULATION sans qu'aucun test d'aller-retour ne le voie — les
//! deux cotes du test passeraient par le meme enveloppeur. L'espion, lui, ne peut RIEN
//! casser : il n'a pas de grille. Prix paye : les octets sont analyses deux fois, mesure
//! dans `tests.rs` (`cout_ingestion_rafale`).
//!
//! ## Cas connus qui ne repassent pas l'aller-retour
//!
//! Ecrits ici plutot que masques — un cas connu coute moins cher qu'un cas cache :
//! - **Les taquets de tabulation** (HTS `ESC H`, TBC `CSI g`) : `Term::tabs` est prive et
//!   HTS pose un taquet A LA COLONNE DU CURSEUR, que l'espion ne connait pas. Ni lisible,
//!   ni restaurable. Aucun des programmes vises (vim, htop, less, claude) n'y touche.
//! - **Le curseur sauvegarde** (DECSC `ESC 7` / DECRC `ESC 8`) : `Grid::saved_cursor` est
//!   public mais le RESTAURER demanderait de deplacer le curseur, de sauvegarder, puis de
//!   revenir — donc d'ecrire dans la grille si le curseur est en butee. Non restaure.
//! - **La grille principale cachee sous l'ecran alternatif** : `swap_alt()` REMET A ZERO
//!   la grille alternative quand on y revient, il n'existe donc aucun moyen de lire la
//!   principale sans detruire l'autre. Decision : quand l'ecran alternatif est actif, le
//!   redessin ne rend que lui. Ce n'est pas une perte pour le service — il tient l'etat
//!   complet en memoire et redessine a nouveau quand l'ecran actif change (tmux repeint
//!   sur le meme evenement). Le test `la_principale_survit_a_l_ecran_alternatif` verrouille
//!   la propriete qui compte : la principale est intacte au retour.
//! - **La palette au-dela de l'index 258** (couleurs « dim », premier plan clair) : aucune
//!   sequence d'echappement ne les pose, seulement la configuration d'Alacritty. Ignorees.
//! - **Le protocole clavier kitty** : desactive (`Config::kitty_keyboard = false`), donc
//!   ses bits de mode ne sont jamais poses et il n'y a rien a restaurer.
//! - **Le curseur en butee a droite sur une cellule de remplissage de caractere large**
//!   (`LEADING_WIDE_CHAR_SPACER` en derniere colonne) : l'etat « attend un retour a la
//!   ligne » n'est pas reproductible sans ecraser la cellule. Configuration inatteignable
//!   en pratique (le curseur est deja passe a la ligne suivante).

/// La PHOTO comparable d'un ecran : elle n'existe que pour le banc, qui serialise, relit
/// dans un emulateur neuf et compare les deux etats. Rien en production n'en a besoin —
/// le service transmet des octets, pas des photos.
#[cfg(test)]
mod etat;
mod redessin;
mod texte;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub use etat::{Cellule, Curseur, EtatEcran};
pub use texte::Occurrence;

use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
#[cfg(test)]
use alacritty_terminal::index::{Column, Line};
#[cfg(test)]
use alacritty_terminal::term::color::COUNT as TAILLE_PALETTE;
use alacritty_terminal::term::{Config, Term, TermMode, MIN_COLUMNS, MIN_SCREEN_LINES};
use alacritty_terminal::vte::ansi::{CharsetIndex, Handler, Processor};

/// Nombre de lignes d'historique gardees par terminal. Aligne sur le `history-limit` que
/// Cockpit posait a tmux : c'est ce que la molette remonte aujourd'hui.
pub const HISTORIQUE: usize = 10_000;

/// Ce que l'emulateur veut faire savoir au monde extERIEUR.
///
/// L'emulateur ne parle a personne : il depose ici, l'appelant ramasse avec
/// `sortants()`. C'est ce qui permet a ce module de ne rien savoir du PTY ni de Tauri.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Sortant {
    /// Le terminal repond au programme (identification, position du curseur...). A
    /// renvoyer TEL QUEL dans l'entree du shell, sinon le programme attend indefiniment.
    VersLeShell(String),
    /// OSC 52 : le programme demande a poser du texte dans le presse-papier systeme.
    VersLePressePapier(String),
}

/// L'ecran d'un terminal : l'etat, et de quoi le redessiner.
pub struct Ecran {
    term: Term<Collecteur>,
    analyseur: Processor,
    /// Ce que `Term` garde prive. Voir l'en-tete du module.
    espion: Espion,
    analyseur_espion: Processor,
    sortants: Arc<Mutex<Vec<Sortant>>>,
}

impl Ecran {
    /// Un ecran a l'historique par defaut. Le service, lui, calcule le sien depuis la
    /// largeur du terminal (`serveur::lignes_d_historique`).
    #[cfg(test)]
    pub fn nouveau(colonnes: usize, lignes: usize) -> Self {
        Self::avec_historique(colonnes, lignes, HISTORIQUE)
    }

    pub fn avec_historique(colonnes: usize, lignes: usize, historique: usize) -> Self {
        let taille = Dimension::assainie(colonnes, lignes);
        let config = Config {
            scrolling_history: historique,
            // Le protocole clavier kitty reste eteint : il pose des bits de mode dans une
            // pile privee, donc non restaurable au redessin, et Cockpit n'en a pas l'usage.
            kitty_keyboard: false,
            ..Config::default()
        };
        let sortants = Arc::new(Mutex::new(Vec::new()));
        Self {
            term: Term::new(config, &taille, Collecteur(Arc::clone(&sortants))),
            analyseur: Processor::new(),
            espion: Espion::neuf(taille.lignes),
            analyseur_espion: Processor::new(),
            sortants,
        }
    }

    /// Donne les octets du shell a l'emulateur.
    pub fn avaler(&mut self, octets: &[u8]) {
        self.analyseur.advance(&mut self.term, octets);
        self.analyseur_espion.advance(&mut self.espion, octets);
    }

    /// Aligne l'ecran sur une nouvelle taille de conteneur.
    pub fn redimensionner(&mut self, colonnes: usize, lignes: usize) {
        let taille = Dimension::assainie(colonnes, lignes);
        self.term.resize(taille);
        // `Term::resize` remet la region de defilement a l'ecran entier ; l'espion suit.
        self.espion.redimensionne(taille.lignes);
    }

    pub fn colonnes(&self) -> usize {
        self.term.columns()
    }

    pub fn lignes(&self) -> usize {
        self.term.screen_lines()
    }

    /// Une application plein ecran occupe le terminal (vim, htop, claude...).
    pub fn ecran_alternatif(&self) -> bool {
        self.term.mode().contains(TermMode::ALT_SCREEN)
    }

    /// Ramasse ce que l'emulateur a a dire, et vide la file.
    pub fn sortants(&self) -> Vec<Sortant> {
        match self.sortants.lock() {
            Ok(mut file) => std::mem::take(&mut *file),
            // Un `Mutex` empoisonne signifie qu'un autre fil a panique en le tenant. On
            // recupere quand meme : perdre une reponse au shell fige le programme.
            Err(empoisonne) => std::mem::take(&mut *empoisonne.into_inner()),
        }
    }

    /// Les octets qui redessinent l'ecran visible, sans l'historique.
    ///
    /// C'est l'equivalent du `capture-pane` de tmux : ce qu'il faut pour que l'utilisateur
    /// retrouve son ecran, sans lui renvoyer 10 000 lignes.
    pub fn redessiner_ecran(&self) -> Vec<u8> {
        redessin::redessiner(self, false)
    }

    /// Les octets qui redessinent l'ecran ET son historique de defilement.
    pub fn redessiner(&self) -> Vec<u8> {
        redessin::redessiner(self, true)
    }

    /// La PHOTO de cet ecran : comme un redessin, mais elle ne bascule jamais en ecran
    /// alternatif. Voir `redessin::photographier` — sans ce mode, un terminal ou un agent
    /// tournait revient vide.
    pub fn photographier(&self) -> Vec<u8> {
        redessin::photographier(self)
    }

    /// La photo comparable de l'etat. Voir `etat.rs`.
    #[cfg(test)]
    pub fn etat(&self) -> EtatEcran {
        self.etat_avec_historique(true)
    }

    /// La photo, historique compris ou non — pour comparer un redessin d'ecran seul.
    #[cfg(test)]
    pub fn etat_avec_historique(&self, avec_historique: bool) -> EtatEcran {
        let grille = self.term.grid();
        let alternatif = self.ecran_alternatif();
        let cellules = self.cellules(avec_historique);
        let curseur = Curseur {
            ligne: grille.cursor.point.line.0,
            colonne: grille.cursor.point.column.0,
            attend_retour: grille.cursor.input_needs_wrap,
            stylo: {
                let mut stylo = Cellule::depuis(&grille.cursor.template);
                // Le stylo n'a pas de caractere : `write_at_cursor` n'en lit que les
                // attributs. Neutralise pour que la comparaison ne porte que sur eux.
                stylo.c = ' ';
                stylo
            },
            forme: self.term.cursor_style().shape,
            clignote: self.term.cursor_style().blinking,
        };
        let palette = (0..TAILLE_PALETTE)
            .filter_map(|i| self.term.colors()[i].map(|rgb| (i, rgb)))
            .collect();
        let jeux = [
            grille.cursor.charsets[CharsetIndex::G0],
            grille.cursor.charsets[CharsetIndex::G1],
            grille.cursor.charsets[CharsetIndex::G2],
            grille.cursor.charsets[CharsetIndex::G3],
        ];
        EtatEcran {
            colonnes: self.colonnes(),
            lignes: self.lignes(),
            alternatif,
            principale: if alternatif { Vec::new() } else { cellules.clone() },
            alternative: if alternatif { cellules } else { Vec::new() },
            curseur,
            region: self.espion.region,
            modes: self.term.mode().bits(),
            palette,
            titre: self.espion.titre.clone(),
            pile_titres: self.espion.pile_titres.clone(),
            jeu_actif: self.espion.jeu_actif as usize,
            jeux,
        }
    }

    /// Les lignes de la grille active, de la plus vieille a la derniere visible.
    #[cfg(test)]
    fn cellules(&self, avec_historique: bool) -> Vec<Vec<Cellule>> {
        let grille = self.term.grid();
        let premiere = if avec_historique {
            grille.topmost_line().0
        } else {
            0
        };
        (premiere..=grille.bottommost_line().0)
            .map(|l| {
                let ligne = &grille[Line(l)];
                (0..self.colonnes())
                    .map(|c| Cellule::depuis(&ligne[Column(c)]))
                    .collect()
            })
            .collect()
    }

    // --- Lectures internes, pour le redessin ---

    pub(super) fn region(&self) -> (i32, i32) {
        self.espion.region
    }

    pub(super) fn titre(&self) -> Option<&str> {
        self.espion.titre.as_deref()
    }

    pub(super) fn pile_titres(&self) -> &[Option<String>] {
        &self.espion.pile_titres
    }

    pub(super) fn jeu_actif(&self) -> CharsetIndex {
        self.espion.jeu_actif
    }

    pub(super) fn term(&self) -> &Term<Collecteur> {
        &self.term
    }
}

/// Une taille de grille, pour `Term::new` et `Term::resize`.
#[derive(Clone, Copy)]
struct Dimension {
    colonnes: usize,
    lignes: usize,
}

impl Dimension {
    /// Borne la taille aux minimums d'alacritty. Une grille de 0 colonne n'est pas une
    /// demande de l'utilisateur mais un conteneur pas encore mesure — silence volontaire.
    fn assainie(colonnes: usize, lignes: usize) -> Self {
        Self {
            colonnes: colonnes.max(MIN_COLUMNS),
            lignes: lignes.max(MIN_SCREEN_LINES),
        }
    }
}

impl Dimensions for Dimension {
    fn total_lines(&self) -> usize {
        self.lignes
    }

    fn screen_lines(&self) -> usize {
        self.lignes
    }

    fn columns(&self) -> usize {
        self.colonnes
    }
}

/// Ramasse ce que l'emulateur veut renvoyer au shell ou au presse-papier.
#[derive(Clone)]
pub(super) struct Collecteur(Arc<Mutex<Vec<Sortant>>>);

impl EventListener for Collecteur {
    fn send_event(&self, evenement: Event) {
        let sortant = match evenement {
            Event::PtyWrite(texte) => Sortant::VersLeShell(texte),
            Event::ClipboardStore(_, texte) => Sortant::VersLePressePapier(texte),
            // Le reste ne concerne pas un service sans fenetre : titre (suivi par
            // l'espion), cloche, curseur qui clignote, demandes de lecture du
            // presse-papier ou de la taille en pixels. Silence VOLONTAIRE, pas un oubli :
            // aucun de ces evenements ne porte d'erreur.
            _ => return,
        };
        match self.0.lock() {
            Ok(mut file) => file.push(sortant),
            Err(empoisonne) => empoisonne.into_inner().push(sortant),
        }
    }
}

/// Le peu que `Term` cache et qu'il faut pour redessiner. Voir l'en-tete du module.
///
/// Toutes les autres methodes de `Handler` gardent leur corps vide par defaut : cet
/// espion n'a pas de grille, il ne peut donc rien emuler et n'essaie pas.
struct Espion {
    /// Hauteur de l'ecran : `set_scrolling_region` en a besoin pour borner comme alacritty.
    lignes: usize,
    /// Region de defilement, bornes d'alacritty (debut inclus, fin exclue).
    region: (i32, i32),
    titre: Option<String>,
    pile_titres: Vec<Option<String>>,
    jeu_actif: CharsetIndex,
}

/// Profondeur de la pile de titres, comme alacritty (`TITLE_STACK_MAX_DEPTH`).
const PILE_TITRES_MAX: usize = 4096;

impl Espion {
    fn neuf(lignes: usize) -> Self {
        Self {
            lignes,
            region: (0, lignes as i32),
            titre: None,
            pile_titres: Vec::new(),
            jeu_actif: CharsetIndex::G0,
        }
    }

    fn redimensionne(&mut self, lignes: usize) {
        self.lignes = lignes;
        self.region = (0, lignes as i32);
    }
}

impl Handler for Espion {
    /// Copie conforme de `Term::set_scrolling_region` : meme valeur par defaut, meme
    /// refus d'une region vide, meme bornage. Toute divergence ici se verrait comme une
    /// region fausse apres redessin.
    fn set_scrolling_region(&mut self, haut: usize, bas: Option<usize>) {
        let bas = bas.unwrap_or(self.lignes);
        if haut >= bas {
            return;
        }
        let plafond = self.lignes as i32;
        self.region = (
            (haut as i32 - 1).min(plafond),
            (bas as i32).min(plafond),
        );
    }

    fn reset_state(&mut self) {
        *self = Espion::neuf(self.lignes);
    }

    fn set_title(&mut self, titre: Option<String>) {
        self.titre = titre;
    }

    fn push_title(&mut self) {
        if self.pile_titres.len() >= PILE_TITRES_MAX {
            return;
        }
        self.pile_titres.push(self.titre.clone());
    }

    fn pop_title(&mut self) {
        if let Some(titre) = self.pile_titres.pop() {
            self.titre = titre;
        }
    }

    fn set_active_charset(&mut self, index: CharsetIndex) {
        self.jeu_actif = index;
    }
}
