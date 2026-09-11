//! Les octets qui redessinent un ecran a l'identique dans un terminal NEUF.
//!
//! C'est la fonction critique du chantier. Elle ne transmet PAS ce que le shell a ecrit :
//! elle decrit ce que l'ecran montre a la fin. Une rafale de 4 Mo qui defile produit donc
//! au plus la taille de la grille, pas 4 Mo (mesure dans `tests.rs`).
//!
//! ## L'ordre compte, et voici pourquoi
//!
//! Chaque etape est placee la ou elle est parce qu'une autre place la casse :
//! 1. `\x1bc` remet le terminal d'arrivee a plat (RIS) ;
//! 2. la palette AVANT le contenu — sinon les cellules deja dessinees garderaient les
//!    anciennes couleurs indexees ;
//! 3. le contenu avec les modes PAR DEFAUT : l'enroulement doit etre actif pour reposer
//!    les fanions `WRAPLINE`, le mode insertion eteint pour ne pas decaler les cellules,
//!    les jeux de caracteres en ASCII pour que les caracteres sortent tels quels ;
//! 4. la region de defilement APRES le contenu — DECSTBM change le point de bascule du
//!    defilement, et il ramene le curseur en haut ;
//! 5. les modes, puis `?6` (origine) : lui aussi ramene le curseur, donc avant de le poser ;
//! 6. le curseur, et seulement ensuite le mode insertion et les jeux de caracteres — ils
//!    modifieraient la retouche de butee decrite plus bas ;
//! 7. le stylo (SGR courant) en dernier : c'est avec lui que la suite s'ecrira.

use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::{Cell, Flags};
use alacritty_terminal::term::color::COUNT as TAILLE_PALETTE;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor, Rgb, StandardCharset};

use super::Ecran;

/// Fanions poses par la MECANIQUE d'ecriture, pas par une sequence SGR : ils se
/// reproduisent en reecrivant les caracteres, jamais en les demandant.
const FANIONS_STRUCTURELS: Flags = Flags::WRAPLINE
    .union(Flags::WIDE_CHAR)
    .union(Flags::WIDE_CHAR_SPACER)
    .union(Flags::LEADING_WIDE_CHAR_SPACER);

pub(super) fn redessiner(ecran: &Ecran, avec_historique: bool) -> Vec<u8> {
    dessiner(ecran, avec_historique, false)
}

/// La PHOTO d'un terminal : le meme dessin, mais qui ne bascule jamais en ecran alternatif.
///
/// **POURQUOI CE MODE EXISTE.** claude, vim et htop dessinent dans l'ecran alternatif, et un
/// redessin ordinaire restitue ce mode : il commence donc par y basculer. Une photo sert a
/// RECOMMENCER un terminal ailleurs, et celui qui la recoit doit revenir a un shell normal —
/// or basculer DETRUIT la grille inactive, donc en sortir laisse un ecran VIDE. Un terminal
/// ou un agent tournait revenait ainsi sans rien, ce qui est le cas le plus courant chez qui
/// en fait tourner un. Mesure par un essai, pas deduit.
///
/// Le contenu de l'ecran alternatif est donc dessine dans l'ecran NORMAL : il devient de
/// l'historique, la molette et la recherche le retrouvent, et le shell neuf ecrit a la suite.
pub(super) fn photographier(ecran: &Ecran) -> Vec<u8> {
    dessiner(ecran, true, true)
}

fn dessiner(ecran: &Ecran, avec_historique: bool, pour_photo: bool) -> Vec<u8> {
    let mut r = Redessin {
        ecran,
        octets: Vec::with_capacity(8 * 1024),
        stylo: Stylo::defaut(),
    };
    r.reinitialiser();
    r.palette();
    r.contenu(avec_historique, pour_photo);
    r.region();
    r.modes();
    r.style_curseur();
    r.curseur();
    r.mode_insertion();
    r.jeux_de_caracteres();
    r.stylo_final();
    r.titres();
    r.octets
}

/// Le stylo : les attributs avec lesquels le prochain caractere s'ecrira.
///
/// Suivi pas a pas pendant le dessin pour n'emettre que les CHANGEMENTS. Sans ca, une
/// grille de 80x24 emettrait 1 920 sequences SGR.
#[derive(Clone, PartialEq, Eq)]
struct Stylo {
    avant: Color,
    arriere: Color,
    /// Deja debarrasse des fanions structurels.
    fanions: Flags,
    soulignement: Option<Color>,
    lien: Option<(String, String)>,
}

impl Stylo {
    fn defaut() -> Self {
        Self {
            avant: Color::Named(NamedColor::Foreground),
            arriere: Color::Named(NamedColor::Background),
            fanions: Flags::empty(),
            soulignement: None,
            lien: None,
        }
    }

    fn depuis(cell: &Cell) -> Self {
        Self {
            avant: cell.fg,
            arriere: cell.bg,
            fanions: cell.flags.difference(FANIONS_STRUCTURELS),
            soulignement: cell.underline_color(),
            lien: cell
                .hyperlink()
                .map(|l| (l.id().to_string(), l.uri().to_string())),
        }
    }

    /// Sans allocation : c'est le test fait pour CHAQUE cellule dessinee.
    fn correspond(&self, cell: &Cell) -> bool {
        self.avant == cell.fg
            && self.arriere == cell.bg
            && self.fanions == cell.flags.difference(FANIONS_STRUCTURELS)
            && self.soulignement == cell.underline_color()
            && match (&self.lien, cell.hyperlink()) {
                (None, None) => true,
                (Some((id, uri)), Some(lien)) => id == lien.id() && uri == lien.uri(),
                _ => false,
            }
    }
}

/// Dans quel etat le dessin d'une ligne laisse le curseur. Ce qui vient apres en depend :
/// un saut de ligne, ou rien du tout parce que l'enroulement va s'en charger.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Arrivee {
    /// Curseur en debut de ligne suivante apres un saut : rien de special.
    Normale,
    /// Curseur en butee a droite : le prochain caractere enroulera, et c'est la SEULE
    /// facon de reposer le fanion `WRAPLINE` de la ligne qu'on vient de dessiner.
    EnButee,
    /// Curseur sur la derniere colonne, laissee vide expres : c'est l'emplacement du
    /// remplissage qu'un caractere large de la ligne suivante y posera en s'y trouvant trop
    /// a l'etroit, et c'est lui qui enroulera.
    AvantCaractereLarge,
}

struct Redessin<'a> {
    ecran: &'a Ecran,
    octets: Vec<u8>,
    stylo: Stylo,
}

impl Redessin<'_> {
    fn pousser(&mut self, texte: &str) {
        self.octets.extend_from_slice(texte.as_bytes());
    }

    /// RIS. Le terminal d'arrivee est normalement neuf, mais deux octets rendent la sortie
    /// utilisable sur un terminal deja sale (une instance xterm.js reutilisee).
    fn reinitialiser(&mut self) {
        self.pousser("\x1bc");
    }

    fn palette(&mut self) {
        let couleurs = self.ecran.term().colors();
        for i in 0..TAILLE_PALETTE {
            let Some(rgb) = couleurs[i] else { continue };
            let sequence = match i {
                0..=255 => format!("\x1b]4;{};{}\x07", i, xcolor(rgb)),
                256 => format!("\x1b]10;{}\x07", xcolor(rgb)),
                257 => format!("\x1b]11;{}\x07", xcolor(rgb)),
                258 => format!("\x1b]12;{}\x07", xcolor(rgb)),
                // Au-dela : couleurs « dim » et premier plan clair, qu'aucune sequence
                // d'echappement ne sait poser. Voir les cas connus en tete du module.
                _ => continue,
            };
            self.pousser(&sequence);
        }
    }

    fn contenu(&mut self, avec_historique: bool, pour_photo: bool) {
        if self.ecran.ecran_alternatif() {
            // On ne redessine que l'ecran alternatif : la principale cachee dessous n'est
            // pas lisible (cas connus, en tete du module). L'historique n'existe pas sur
            // l'ecran alternatif.
            //
            // Pour une PHOTO, on ne bascule PAS : voir `photographier`. Le contenu part dans
            // l'ecran normal, donc il survit au retour a un shell.
            if !pour_photo {
                self.pousser("\x1b[?1049h");
            }
            self.dessiner_grille(false);
        } else {
            self.dessiner_grille(avec_historique);
        }
    }

    fn dessiner_grille(&mut self, avec_historique: bool) {
        let grille = self.ecran.term().grid();
        let premiere = if avec_historique {
            grille.topmost_line().0
        } else {
            0
        };
        let derniere = grille.bottommost_line().0;
        let colonnes = self.ecran.colonnes();

        self.pousser("\x1b[H");
        let mut arrivee = Arrivee::Normale;
        for l in premiere..=derniere {
            let enroulee = grille[Line(l)][Column(colonnes - 1)]
                .flags
                .contains(Flags::WRAPLINE);
            let fin = self.fin_de_ligne(l, enroulee);
            // La ligne rend DANS QUEL ETAT elle laisse le curseur, et ce n'est pas la meme
            // chose que « la ligne est enroulee » : sa derniere cellule peut avoir ete
            // sautee (caractere large decale, remplissage d'un caractere large de la ligne
            // suivante). Enchainer sur un enroulement qui n'aura pas lieu ecrirait la ligne
            // d'apres PAR-DESSUS celle-ci.
            arrivee = self.dessiner_ligne(l, fin, arrivee);
            if !enroulee {
                // La ligne n'est PAS enroulee : meme si le curseur est en butee (une ligne
                // pleine qui n'a pas deborde), il faut un saut de ligne. Sans ca le premier
                // caractere de la suivante poserait un `WRAPLINE` que la source n'a pas.
                arrivee = Arrivee::Normale;
            }
            if l == derniere {
                continue;
            }
            // Le fond du stylo est HERITE par toute ligne qui entre par le bas
            // (`Cell::reset` ne recopie que `bg`). Sans ce retour au fond par defaut, les
            // fins de ligne rognees ressortiraient teintees.
            if self.stylo.arriere != Color::Named(NamedColor::Background) {
                self.pousser("\x1b[49m");
                self.stylo.arriere = Color::Named(NamedColor::Background);
            }
            if arrivee == Arrivee::Normale {
                self.pousser("\r\n");
            }
            // Sinon : PAS de saut de ligne. C'est le prochain caractere qui provoquera
            // l'enroulement, seule facon de reposer `WRAPLINE` et le remplissage d'un
            // caractere large a cheval.
        }
    }

    /// Jusqu'ou il faut dessiner cette ligne. Les fins de ligne vierges ne sont pas
    /// redessinees — un terminal neuf les a deja, et c'est ce rognage qui fait qu'un ecran
    /// vide coute quelques octets. Une ligne enroulee, elle, doit aller jusqu'a la
    /// derniere colonne : c'est son ecriture qui provoque l'enroulement.
    fn fin_de_ligne(&self, ligne: i32, enroulee: bool) -> usize {
        let mut fin = self.ecran.colonnes();
        if enroulee {
            return fin;
        }
        let grille = self.ecran.term().grid();
        while fin > 0 && est_vierge(&grille[Line(ligne)][Column(fin - 1)]) {
            fin -= 1;
        }
        fin
    }

    /// Cette cellule est-elle un remplissage que le caractere large voisin reposera tout
    /// seul ? Si oui, il ne faut PAS l'ecrire — le caractere large s'en charge.
    ///
    /// Le voisinage est verifie, et ce n'est pas de la prudence gratuite : un effacement
    /// (`CSI X`, `CSI P`) peut retirer le caractere large en laissant son remplissage
    /// derriere lui. Sauter un remplissage ORPHELIN ferait pire que perdre son fanion : le
    /// curseur resterait en arriere et tout le reste de la ligne glisserait d'une colonne.
    fn remplissage_a_ignorer(&self, ligne: i32, colonne: usize) -> bool {
        let grille = self.ecran.term().grid();
        let fanions = grille[Line(ligne)][Column(colonne)].flags;
        if fanions.contains(Flags::WIDE_CHAR_SPACER) {
            return colonne > 0
                && grille[Line(ligne)][Column(colonne - 1)]
                    .flags
                    .contains(Flags::WIDE_CHAR);
        }
        if fanions.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
            // Pose par le caractere large de la ligne SUIVANTE, faute de place ici.
            let suivante = ligne + 1;
            return suivante <= grille.bottommost_line().0
                && grille[Line(suivante)][Column(0)]
                    .flags
                    .contains(Flags::WIDE_CHAR);
        }
        false
    }

    fn stylo_par_defaut(&mut self) {
        if self.stylo != Stylo::defaut() {
            self.pousser("\x1b[m");
            if self.stylo.lien.is_some() {
                self.pousser("\x1b]8;;\x07");
            }
            self.stylo = Stylo::defaut();
        }
    }

    /// Dessine une ligne en SUIVANT la colonne du curseur, pas en supposant qu'elle avance
    /// d'un cran par cellule.
    ///
    /// Un caractere large en mange deux, une tabulation saute, et une grille malmenee par
    /// des insertions/suppressions peut contenir un caractere large dont le remplissage a
    /// disparu. Sans ce suivi, un seul de ces cas decalait TOUTE la fin de la ligne d'une
    /// colonne — la corruption la plus visible qu'on puisse produire. Le curseur suivi est
    /// recale par `CSI G` des qu'il s'ecarte de la colonne a ecrire.
    fn dessiner_ligne(&mut self, ligne: i32, fin: usize, arrivee: Arrivee) -> Arrivee {
        let colonnes = self.ecran.colonnes();
        let mut curseur = 0usize;
        let mut en_butee = arrivee == Arrivee::EnButee;
        for c in 0..fin {
            let cell = &self.ecran.term().grid()[Line(ligne)][Column(c)];
            let (caractere, fanions, arriere) = (cell.c, cell.flags, cell.bg);
            let tabulation = caractere == '\t';
            let couvert = curseur > c;
            let sur_remplissage = self.remplissage_a_ignorer(ligne, c);

            if tabulation {
                // Une colonne recouverte par un caractere large ne se reecrit pas — sauf
                // si c'est SON remplissage : la tabulation s'y est posee par-dessus et on
                // peut la reposer de meme, sans y remettre d'espace.
                if couvert && !sur_remplissage {
                    continue;
                }
                if sur_remplissage && fanions.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
                    // Ce remplissage-la n'existe pas encore : c'est le caractere large de la
                    // ligne SUIVANTE qui le posera, et en le posant il y ecrira une espace.
                    // La tabulation serait effacee juste apres — on ne l'ecrit pas.
                    continue;
                }
            } else {
                if couvert {
                    // Colonne mangee par le caractere large qui precede.
                    continue;
                }
                if sur_remplissage {
                    continue;
                }
                if fanions.contains(Flags::WIDE_CHAR) && c + 1 >= colonnes {
                    // Un caractere large ne peut pas NAITRE en derniere colonne : il y
                    // pose un remplissage et passe a la ligne. En trouver un ici veut dire
                    // qu'une suppression de caracteres l'y a decale. Le reecrire ferait
                    // enrouler la ligne et decalerait tout ce qui suit : on le laisse.
                    continue;
                }
            }

            // `poser_espace` : `put_tab` ecrit `\t` dans toute cellule qui contient une
            // espace, remplissage compris. Y reposer une espace ecraserait le caractere
            // large d'a cote (`write_at_cursor` nettoie la paire).
            let poser_espace = !sur_remplissage && !couvert;
            let positionne = curseur != c || (tabulation && !poser_espace);
            // Deux raisons de PROVOQUER l'enroulement plutot que de le laisser venir :
            // - il faut sortir de la butee avant toute sequence de positionnement, et le
            //   seul geste qui consomme une butee est l'ecriture d'un caractere ;
            // - l'enroulement fait DEFILER, et la ligne qui entre par le bas herite du fond
            //   du stylo (`Cell::reset` ne recopie que `bg`). Si la premiere cellule a
            //   ecrire porte un fond, ce fond teinterait toute la ligne d'arrivee, y compris
            //   les fins de ligne qu'on ne redessine pas.
            let fond_teinte = arriere != Color::Named(NamedColor::Background);
            if en_butee && (positionne || fond_teinte) {
                self.forcer_enroulement();
                curseur = 0;
            }
            if curseur != c {
                self.pousser(&format!("\x1b[{}G", c + 1));
            }

            if tabulation {
                self.ecrire_tabulation(ligne, c, poser_espace);
                // En DERNIERE colonne, `put_tab` ecrit mais n'avance pas : le curseur reste
                // dessus, sans butee. Compter une colonne de plus ferait croire a un
                // enroulement a venir et la ligne suivante ecraserait celle-ci.
                curseur = if c + 1 < colonnes { c + 1 } else { c };
            } else {
                self.ecrire_cellule(ligne, c);
                curseur = c + largeur_affichee(caractere);
            }
            en_butee = false;
        }

        // Rien n'a ete ecrit et la ligne d'avant etait enroulee : sans un caractere pour
        // le declencher, son `WRAPLINE` ne serait jamais pose.
        if en_butee {
            self.forcer_enroulement();
            curseur = 0;
        }

        if curseur >= colonnes {
            Arrivee::EnButee
        } else if curseur == colonnes - 1
            && self.remplissage_a_ignorer(ligne, colonnes - 1)
            && self.ecran.term().grid()[Line(ligne)][Column(colonnes - 1)]
                .flags
                .contains(Flags::LEADING_WIDE_CHAR_SPACER)
        {
            // La derniere cellule est le remplissage qu'un caractere large de la ligne
            // SUIVANTE viendra poser en s'y trouvant trop a l'etroit. On laisse donc le
            // curseur dessus : c'est ce caractere large qui fera l'enroulement.
            Arrivee::AvantCaractereLarge
        } else {
            Arrivee::Normale
        }
    }

    /// Consomme une butee a droite : une espace au stylo par defaut (donc une cellule qui
    /// reste vierge) suivie d'un retour chariot, qui ramene en colonne 0 de la ligne ou
    /// l'enroulement vient de nous amener.
    fn forcer_enroulement(&mut self) {
        self.stylo_par_defaut();
        self.pousser(" \r");
    }

    /// La TABULATION est le seul caractere de commande qui finit DANS une cellule, et elle
    /// ne s'y met pas comme les autres : `put_tab` n'y ecrit `\t` que si la cellule contient
    /// deja une espace, ne touche pas ses attributs, et saute au taquet suivant.
    ///
    /// Emettre `\t` a la place du caractere ne marche donc pas : le curseur partirait de huit
    /// colonnes et tout le reste de la ligne glisserait. On pose donc une espace avec les
    /// bons attributs, on revient dessus, on tabule, puis on revient a la colonne suivante.
    fn ecrire_tabulation(&mut self, ligne: i32, colonne: usize, poser_espace: bool) {
        if poser_espace {
            self.ecrire_caractere(ligne, colonne, ' ');
        }
        self.pousser(&format!("\x1b[{}G\t", colonne + 1));
        if colonne + 1 < self.ecran.colonnes() {
            self.pousser(&format!("\x1b[{}G", colonne + 2));
        }
    }

    fn ecrire_cellule(&mut self, ligne: i32, colonne: usize) {
        let c = self.ecran.term().grid()[Line(ligne)][Column(colonne)].c;
        self.ecrire_caractere(ligne, colonne, c);
    }

    /// Ecrit `c` avec les attributs de la cellule (`c` differe d'elle pour la tabulation).
    fn ecrire_caractere(&mut self, ligne: i32, colonne: usize, c: char) {
        let grille = self.ecran.term().grid();
        let cell = &grille[Line(ligne)][Column(colonne)];
        if !self.stylo.correspond(cell) {
            let voulu = Stylo::depuis(cell);
            let sequence = self.sequence_stylo(&voulu);
            self.pousser(&sequence);
            self.stylo = voulu;
        }
        let mut texte = String::with_capacity(4);
        texte.push(c);
        for combinant in cell.zerowidth().unwrap_or(&[]) {
            texte.push(*combinant);
        }
        self.pousser(&texte);
    }

    /// Les octets qui font passer le stylo courant a `voulu`.
    ///
    /// Deux chemins : un ajout pur (le cas courant, quelques octets) ou une remise a plat
    /// `SGR 0` suivie de tous les attributs, des qu'il faut RETIRER quelque chose — il
    /// n'existe pas de « retirer le gras et l'italique » en une passe qui ne risque pas de
    /// laisser un residu.
    fn sequence_stylo(&self, voulu: &Stylo) -> String {
        let retire = self.stylo.fanions.difference(voulu.fanions);
        let couleurs_changent = self.stylo.avant != voulu.avant
            || self.stylo.arriere != voulu.arriere
            || self.stylo.soulignement != voulu.soulignement;
        let mut sortie = String::new();

        let mut params: Vec<String> = Vec::new();
        if retire.is_empty() && !couleurs_changent {
            for p in params_fanions(voulu.fanions.difference(self.stylo.fanions)) {
                params.push(p);
            }
        } else {
            params.push("0".into());
            params.extend(params_fanions(voulu.fanions));
            if voulu.avant != Color::Named(NamedColor::Foreground) {
                params.push(param_couleur(voulu.avant, true));
            }
            if voulu.arriere != Color::Named(NamedColor::Background) {
                params.push(param_couleur(voulu.arriere, false));
            }
            if let Some(couleur) = voulu.soulignement {
                params.push(param_soulignement(couleur));
            }
        }
        if !params.is_empty() {
            sortie.push_str(&format!("\x1b[{}m", params.join(";")));
        }

        if self.stylo.lien != voulu.lien {
            sortie.push_str(&match &voulu.lien {
                Some((id, uri)) => format!("\x1b]8;id={id};{uri}\x07"),
                None => "\x1b]8;;\x07".to_string(),
            });
        }
        sortie
    }

    fn region(&mut self) {
        let (debut, fin) = self.ecran.region();
        if (debut, fin) == (0, self.ecran.lignes() as i32) {
            return;
        }
        // DECSTBM compte les lignes a partir de 1 ; alacritty garde un debut INCLUS a
        // partir de 0 et une fin EXCLUE, d'ou le +1 sur le seul debut.
        //
        // Le `max` n'est pas de la prudence : `set_scrolling_region` REFUSE une region ou
        // le haut n'est pas strictement au-dessus du bas, et il BORNE ensuite les deux a la
        // hauteur de l'ecran. Une region collee au bas de l'ecran (par exemple 23..24 sur
        // 24 lignes) ne se demande donc pas « 24;24 » — ce serait refuse — mais « 24;25 »,
        // que le bornage ramene a 24. Trouve par les octets au hasard.
        let bas = fin.max(debut + 2);
        self.pousser(&format!("\x1b[{};{}r", debut + 1, bas));
    }

    fn modes(&mut self) {
        let actuels = *self.ecran.term().mode();
        let defaut = TermMode::default();

        // L'ordre des modes souris n'est pas libre : chacun des trois EFFACE les deux
        // autres quand on le pose (`MOUSE_MODE` est retire d'abord). Ils sont donc
        // exclusifs, et on ne pose que celui qui l'est.
        for (bit, code) in [
            (TermMode::MOUSE_MOTION, 1003),
            (TermMode::MOUSE_DRAG, 1002),
            (TermMode::MOUSE_REPORT_CLICK, 1000),
        ] {
            if actuels.contains(bit) {
                self.pousser(&format!("\x1b[?{code}h"));
            }
        }

        for (bit, code) in [
            (TermMode::APP_CURSOR, 1),
            (TermMode::LINE_WRAP, 7),
            (TermMode::SHOW_CURSOR, 25),
            (TermMode::FOCUS_IN_OUT, 1004),
            (TermMode::UTF8_MOUSE, 1005),
            (TermMode::SGR_MOUSE, 1006),
            (TermMode::ALTERNATE_SCROLL, 1007),
            (TermMode::URGENCY_HINTS, 1042),
            (TermMode::BRACKETED_PASTE, 2004),
        ] {
            if actuels.contains(bit) == defaut.contains(bit) {
                continue;
            }
            let action = if actuels.contains(bit) { 'h' } else { 'l' };
            self.pousser(&format!("\x1b[?{code}{action}"));
        }

        if actuels.contains(TermMode::LINE_FEED_NEW_LINE) {
            self.pousser("\x1b[20h");
        }
        if actuels.contains(TermMode::APP_KEYPAD) {
            self.pousser("\x1b=");
        }
        // `?6h` ramene le curseur en haut de la region : il doit passer AVANT le
        // positionnement, et le positionnement devient alors relatif a la region.
        if actuels.contains(TermMode::ORIGIN) {
            self.pousser("\x1b[?6h");
        }
    }

    fn style_curseur(&mut self) {
        let style = self.ecran.term().cursor_style();
        let code = match (style.shape, style.blinking) {
            (CursorShape::Block, false) => return, // deja l'etat d'un terminal neuf
            (CursorShape::Block, true) => 1,
            (CursorShape::Underline, true) => 3,
            (CursorShape::Underline, false) => 4,
            (CursorShape::Beam, true) => 5,
            (CursorShape::Beam, false) => 6,
            // `HollowBlock` et `Hidden` ne viennent que de la configuration d'Alacritty,
            // aucune sequence ne les pose : retour au style par defaut.
            _ => 0,
        };
        self.pousser(&format!("\x1b[{code} q"));
    }

    fn curseur(&mut self) {
        let curseur = self.ecran.term().grid().cursor.point;
        self.poser_curseur(curseur.line.0, curseur.column.0);
        self.retouche_butee();
    }

    /// Positionne le curseur, en tenant compte du MODE ORIGINE : quand il est actif, `CUP`
    /// compte les lignes depuis le haut de la region de defilement, pas depuis le haut de
    /// l'ecran. Le mode est pose juste avant, donc toute mise en place passe par ici.
    fn poser_curseur(&mut self, ligne: i32, colonne: usize) {
        let ligne = if self.ecran.term().mode().contains(TermMode::ORIGIN) {
            ligne - self.ecran.region().0
        } else {
            ligne
        };
        self.pousser(&format!("\x1b[{};{}H", ligne + 1, colonne + 1));
    }

    /// Le curseur « en butee a droite » : le prochain caractere passe a la ligne AVANT
    /// d'etre ecrit. Aucune sequence de positionnement ne pose cet etat — seule l'ecriture
    /// d'un caractere en derniere colonne le fait. On reecrit donc la cellule qui s'y
    /// trouve, a l'identique : la grille ne change pas, l'etat de butee revient.
    /// Repose l'etat « le curseur est en butee a droite : le prochain caractere passe a la
    /// ligne AVANT d'etre ecrit ». Aucune sequence de positionnement ne le donne — seule
    /// l'ecriture d'un caractere en derniere colonne le produit, donc on reecrit la cellule
    /// qui s'y trouve, a l'identique.
    ///
    /// CE QUI SE PERD : cette reecriture repose les attributs du stylo, donc elle EFFACE le
    /// `WRAPLINE` de la cellule si elle en avait un. Un curseur en butee SUR une ligne
    /// enroulee n'est atteignable qu'en sauvegardant puis restaurant le curseur (`ESC 7` /
    /// `ESC 8`) autour d'un enroulement — aucun programme reel ne le fait, et le fanion
    /// perdu ne concerne qu'une ligne dont on connait deja la suite.
    fn retouche_butee(&mut self) {
        let curseur = self.ecran.term().grid().cursor.clone();
        if !curseur.input_needs_wrap {
            return;
        }
        let ligne = curseur.point.line.0;
        let derniere = self.ecran.colonnes() - 1;
        // La butee ne se pose qu'en ecrivant en DERNIERE colonne. Quand le curseur est
        // ailleurs, c'est qu'un retour de tabulation (`CSI Z`) l'a ramene en arriere sans
        // effacer la butee — c'est le SEUL geste d'alacritty qui deplace la colonne sans la
        // remettre a zero. On refait donc les deux gestes, dans l'ordre.
        if curseur.point.column.0 != derniere
            && retours_de_tabulation(derniere, curseur.point.column.0).is_none()
        {
            // Butee sur une colonne qu'aucun retour de tabulation n'atteint : etat
            // inatteignable avec les taquets par defaut (les seuls qu'on restaure).
            return;
        }

        let fanions = self.ecran.term().grid()[Line(ligne)][Column(derniere)].flags;
        if fanions.contains(Flags::LEADING_WIDE_CHAR_SPACER) {
            // Cas connu, non reproductible : reecrire cette cellule effacerait le
            // remplissage. Inatteignable en pratique (le curseur est deja a la ligne
            // suivante quand ce remplissage existe).
            return;
        }
        // Si la cellule d'AVANT porte un caractere large, alors le redessin a mis un
        // remplissage en derniere colonne. Y ecrire ferait nettoyer la paire par
        // `write_at_cursor`, donc EFFACERAIT le caractere large : c'est lui qu'il faut
        // reecrire, il reposera son remplissage et laissera le curseur en butee.
        //
        // La question porte sur la cellule d'avant et non sur le fanion de remplissage de
        // la derniere : un effacement peut avoir retire le remplissage de la SOURCE alors
        // que le redessin, lui, le recree forcement.
        let colonne = if derniere > 0
            && self.ecran.term().grid()[Line(ligne)][Column(derniere - 1)]
                .flags
                .contains(Flags::WIDE_CHAR)
        {
            derniere - 1
        } else {
            derniere
        };
        self.poser_curseur(ligne, colonne);
        self.ecrire_cellule(ligne, colonne);
        self.completer_par_retours_de_tabulation(curseur.point.column.0);
    }

    /// Ramene le curseur de la derniere colonne a `colonne` par des retours de tabulation,
    /// les seuls a ne pas annuler la butee.
    fn completer_par_retours_de_tabulation(&mut self, colonne: usize) {
        let derniere = self.ecran.colonnes() - 1;
        if colonne == derniere {
            return;
        }
        if let Some(retours) = retours_de_tabulation(derniere, colonne) {
            self.pousser(&format!("\x1b[{retours}Z"));
        }
    }

    /// Le mode insertion decale les cellules a droite a chaque caractere : il ne peut etre
    /// pose qu'apres tout ce qui ecrit, retouche de butee comprise.
    fn mode_insertion(&mut self) {
        if self.ecran.term().mode().contains(TermMode::INSERT) {
            self.pousser("\x1b[4h");
        }
    }

    /// Les jeux de caracteres TRADUISENT ce qu'on ecrit (le mode trace de lignes rend
    /// `q` en `─`) : ils viennent apres tout le contenu, sinon celui-ci sortirait traduit.
    fn jeux_de_caracteres(&mut self) {
        let jeux = self.ecran.term().grid().cursor.charsets;
        for (index, intermediaire) in [
            (alacritty_terminal::vte::ansi::CharsetIndex::G0, '('),
            (alacritty_terminal::vte::ansi::CharsetIndex::G1, ')'),
            (alacritty_terminal::vte::ansi::CharsetIndex::G2, '*'),
            (alacritty_terminal::vte::ansi::CharsetIndex::G3, '+'),
        ] {
            let final_ = match jeux[index] {
                StandardCharset::Ascii => 'B',
                StandardCharset::SpecialCharacterAndLineDrawing => '0',
            };
            self.pousser(&format!("\x1b{intermediaire}{final_}"));
        }
        match self.ecran.jeu_actif() {
            alacritty_terminal::vte::ansi::CharsetIndex::G0 => self.pousser("\x0f"),
            alacritty_terminal::vte::ansi::CharsetIndex::G1 => self.pousser("\x0e"),
            // G2 et G3 ne s'invoquent que par SS2/SS3, qui ne valent que pour UN
            // caractere : `set_active_charset` ne peut pas s'y arreter durablement.
            _ => {}
        }
    }

    fn stylo_final(&mut self) {
        let voulu = Stylo::depuis(&self.ecran.term().grid().cursor.template);
        // Remise a plat SANS condition : le stylo final est ce avec quoi TOUTE la suite
        // s'ecrira, il doit etre exact et pas approche. Quelques octets pour une certitude.
        let mut params = vec!["0".to_string()];
        params.extend(params_fanions(voulu.fanions));
        if voulu.avant != Color::Named(NamedColor::Foreground) {
            params.push(param_couleur(voulu.avant, true));
        }
        if voulu.arriere != Color::Named(NamedColor::Background) {
            params.push(param_couleur(voulu.arriere, false));
        }
        if let Some(couleur) = voulu.soulignement {
            params.push(param_soulignement(couleur));
        }
        self.pousser(&format!("\x1b[{}m", params.join(";")));
        let lien = match &voulu.lien {
            Some((id, uri)) => format!("\x1b]8;id={id};{uri}\x07"),
            None => "\x1b]8;;\x07".to_string(),
        };
        self.pousser(&lien);
        self.stylo = voulu;
    }

    /// La pile de titres se refabrique en empilant : `push_title` empile le titre COURANT,
    /// donc on pose chaque titre puis on empile, et on finit par le titre actuel.
    fn titres(&mut self) {
        for empile in self.ecran.pile_titres().to_vec() {
            if let Some(titre) = empile {
                self.pousser(&format!("\x1b]2;{titre}\x07"));
            }
            self.pousser("\x1b[22t");
        }
        if let Some(titre) = self.ecran.titre() {
            self.pousser(&format!("\x1b]2;{titre}\x07"));
        }
    }
}

/// La cellule d'un ecran vierge, au sens du rognage : un terminal neuf l'a deja.
///
/// Ne compare pas un `Cell` a `Cell::default()` : `clear_wide` peut laisser un stockage
/// annexe present mais VIDE, qui n'est pas egal a l'absence de stockage alors qu'il decrit
/// la meme cellule.
fn est_vierge(cell: &Cell) -> bool {
    cell.c == ' '
        && cell.fg == Color::Named(NamedColor::Foreground)
        && cell.bg == Color::Named(NamedColor::Background)
        && cell.flags.is_empty()
        && cell.zerowidth().unwrap_or(&[]).is_empty()
        && cell.underline_color().is_none()
        && cell.hyperlink().is_none()
}

fn params_fanions(fanions: Flags) -> Vec<String> {
    let mut params = Vec::new();
    for (bit, param) in [
        (Flags::BOLD, "1"),
        (Flags::DIM, "2"),
        (Flags::ITALIC, "3"),
        (Flags::UNDERLINE, "4"),
        (Flags::DOUBLE_UNDERLINE, "4:2"),
        (Flags::UNDERCURL, "4:3"),
        (Flags::DOTTED_UNDERLINE, "4:4"),
        (Flags::DASHED_UNDERLINE, "4:5"),
        (Flags::INVERSE, "7"),
        (Flags::HIDDEN, "8"),
        (Flags::STRIKEOUT, "9"),
    ] {
        if fanions.contains(bit) {
            params.push(param.to_string());
        }
    }
    params
}

/// Le parametre SGR d'une couleur de cellule.
fn param_couleur(couleur: Color, avant: bool) -> String {
    let base = if avant { 30 } else { 40 };
    match couleur {
        Color::Named(nom) => {
            let index = nom as usize;
            match index {
                0..=7 => format!("{}", base + index),
                8..=15 => format!("{}", base + 60 + index - 8),
                256 if avant => "39".into(),
                257 if !avant => "49".into(),
                // Les autres entrees nommees (curseur, couleurs « dim ») ne peuvent pas
                // etre la couleur d'une cellule : aucune sequence SGR ne les pose. Repli
                // sur la couleur indexee de meme rang pour ne rien inventer de faux.
                _ => format!("{};5;{}", base + 8, index.min(255)),
            }
        }
        Color::Indexed(i) => format!("{};5;{}", base + 8, i),
        Color::Spec(rgb) => format!("{};2;{};{};{}", base + 8, rgb.r, rgb.g, rgb.b),
    }
}

fn param_soulignement(couleur: Color) -> String {
    match couleur {
        Color::Spec(rgb) => format!("58;2;{};{};{}", rgb.r, rgb.g, rgb.b),
        Color::Indexed(i) => format!("58;5;{i}"),
        Color::Named(nom) => format!("58;5;{}", (nom as usize).min(255)),
    }
}

/// Intervalle des taquets de tabulation par defaut, comme `INITIAL_TABSTOPS` chez
/// alacritty. Ce sont les seuls qu'on restaure (les taquets poses par HTS ne sont pas
/// lisibles depuis `Term`), donc les seuls sur lesquels compter ici.
const TAQUET: usize = 8;

/// Combien de retours de tabulation (`CSI Z`) menent de `depart` a `arrivee`, ou `None` si
/// aucun n'y arrive exactement.
fn retours_de_tabulation(depart: usize, arrivee: usize) -> Option<usize> {
    let mut colonne = depart;
    for retours in 1..=depart {
        colonne = (0..colonne).rev().find(|i| i % TAQUET == 0)?;
        if colonne == arrivee {
            return Some(retours);
        }
        if colonne < arrivee {
            return None;
        }
    }
    None
}

/// Le nombre de COLONNES qu'occupe un caractere, compte exactement comme l'emulateur.
///
/// `unicode-width` rend parfois 3 — le signe khmer U+17D8 par exemple. `Term::input` ne
/// connait que deux cas : « une colonne » (largeur 1) et « deux colonnes » (tout le reste).
/// Recopier ce choix n'est pas un detail de style : compter 3 la ou l'emulateur compte 2
/// sautait une cellule et decalait toute la fin de la ligne (trouve par les octets au
/// hasard, invisible sur les traces reelles).
fn largeur_affichee(c: char) -> usize {
    match unicode_width::UnicodeWidthChar::width(c) {
        None | Some(0) | Some(1) => 1,
        Some(_) => 2,
    }
}

/// Une couleur au format XParseColor, le seul que l'analyseur accepte pour les OSC.
fn xcolor(rgb: Rgb) -> String {
    format!("rgb:{:02x}/{:02x}/{:02x}", rgb.r, rgb.g, rgb.b)
}
