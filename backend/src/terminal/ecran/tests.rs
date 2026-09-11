//! Le test qui decide de tout : l'ALLER-RETOUR.
//!
//! Pour n'importe quel etat : serialiser, relire le resultat dans un emulateur NEUF,
//! comparer les deux etats — cellule par cellule, attributs compris, plus le curseur,
//! l'ecran actif, la region de defilement et les modes DEC. C'est une egalite, donc c'est
//! automatisable : c'est ce qui transforme la fidelite d'un emulateur de terminal en un
//! travail borne au lieu d'une suite de correctifs sans fin.
//!
//! Trois sources d'entrees, de la moins a la plus precieuse :
//! 1. des etats fabriques a la main, un par propriete a verrouiller ;
//! 2. des octets au hasard — n'importe quelle suite d'octets donne un etat valide, donc
//!    n'importe quelle suite est un cas de test ;
//! 3. de VRAIES traces (`src-tauri/tests/traces/`), captees dans un PTY de 80x24 avec
//!    `scripts/capturer-trace.py`. C'est la seule source qui contient ce que personne
//!    n'aurait pense a ecrire.

use super::{Cellule, Ecran, EtatEcran, Sortant, HISTORIQUE};

const COLONNES: usize = 80;
const LIGNES: usize = 24;

// --- Les traces, embarquees a la compilation : aucun chemin a resoudre a l'execution ---

const TRACE_VIM: &[u8] = include_bytes!("../../../tests/traces/vim.raw");
const TRACE_HTOP: &[u8] = include_bytes!("../../../tests/traces/htop.raw");
const TRACE_LESS: &[u8] = include_bytes!("../../../tests/traces/less.raw");
const TRACE_GITLOG: &[u8] = include_bytes!("../../../tests/traces/gitlog.raw");
const TRACE_LS: &[u8] = include_bytes!("../../../tests/traces/ls.raw");
const TRACE_CLAUDE: &[u8] = include_bytes!("../../../tests/traces/claude.raw");

fn traces() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("vim", TRACE_VIM),
        ("htop", TRACE_HTOP),
        ("less", TRACE_LESS),
        ("git log", TRACE_GITLOG),
        ("ls --color", TRACE_LS),
        ("claude", TRACE_CLAUDE),
    ]
}

// --- Le harnais ---

fn ecran_avale(octets: &[u8]) -> Ecran {
    let mut ecran = Ecran::avec_historique(COLONNES, LIGNES, HISTORIQUE);
    ecran.avaler(octets);
    ecran
}

/// Serialise, relit dans un emulateur neuf, et exige l'egalite EXACTE.
fn verifier(nom: &str, octets: &[u8]) {
    assert!(
        !verifier_tolerant(nom, octets),
        "« {nom} » ne repasse l'aller-retour qu'au prix d'un indice de largeur orphelin. \
         Sur une entree reelle, ce n'est pas une tolerance : c'est un bug."
    );
}

/// Le meme test, mais qui REND `true` au lieu d'echouer quand la seule divergence est un
/// indice de largeur orphelin (voir `oublier_indices_de_largeur_orphelins`). Reserve aux
/// tirages au hasard : eux seuls fabriquent ces etats.
fn verifier_tolerant(nom: &str, octets: &[u8]) -> bool {
    let mut tolere = false;
    for avec_historique in [false, true] {
        let source = ecran_avale(octets);
        let redessin = if avec_historique {
            source.redessiner()
        } else {
            source.redessiner_ecran()
        };
        let relu = ecran_avale(&redessin);

        let attendu = source.etat_avec_historique(avec_historique);
        let obtenu = relu.etat_avec_historique(avec_historique);
        let Some(quoi) = attendu.difference(&obtenu) else {
            continue;
        };
        let mut attendu_nettoye = attendu.clone();
        let mut obtenu_nettoye = obtenu.clone();
        mettre_de_cote_les_cas_connus(&mut attendu_nettoye, &mut obtenu_nettoye);
        let Some(residu) = attendu_nettoye.difference(&obtenu_nettoye) else {
            tolere = true;
            continue;
        };
        panic!(
            "aller-retour casse sur « {nom} » (historique: {avec_historique})\n  {quoi}\n\
             \n  ce qui reste apres avoir mis de cote les fanions tolerables :\n  {residu}\n\
             \n  redessin ({} octets) : {:?}",
            redessin.len(),
            apercu(&redessin)
        );
    }
    tolere
}

/// Un debut de redessin lisible dans un message d'echec, sans noyer la sortie.
fn apercu(octets: &[u8]) -> String {
    let fin = octets.len().min(400);
    String::from_utf8_lossy(&octets[..fin])
        .escape_debug()
        .to_string()
}

/// Les seules divergences acceptees, et seulement sur des octets au hasard. Chacune est
/// nommee par sa CAUSE, pas par son symptome — une tolerance vague finirait par cacher un
/// vrai bug. Toutes tournent autour d'une meme famille : une grille malmenee par des
/// insertions et des effacements peut contenir des restes de caracteres larges qui ne
/// designent plus rien, et qu'aucune sequence d'echappement ne sait reposer tels quels.
///
/// 1. **Indice de largeur orphelin, sur une cellule blanche.** Un effacement (`CSI X`,
///    `CSI P`) peut retirer un caractere large en laissant derriere lui la cellule de
///    remplissage qui l'accompagnait. Ce remplissage devient un indice de largeur qui ne
///    designe plus rien : une cellule BLANCHE qui annonce « je suis la moitie d'un
///    caractere large ». Le redessin la repose comme une espace ordinaire, donc sans le
///    fanion — aucune sequence ne sait le poser sans remettre le caractere large avec. Ce
///    qui se perd : rien de visible, la cellule est vide des deux cotes. « Vide » couvre
///    l'espace ET la tabulation, comme `Cell::is_empty` chez alacritty : une tabulation
///    peut atterrir dans un remplissage, qui contient justement une espace.
/// 2. **`WRAPLINE` la ou il est inerte ou impossible a reposer.** Ce fanion ne se pose
///    qu'en ecrivant un caractere alors que le curseur est en butee a droite, et il ne veut
///    dire quelque chose que sur la DERNIERE COLONNE (la seule que lisent le recadrage et
///    la copie). Quatre etats echappent donc au redessin :
///    - sur une autre colonne, ou une suppression de caracteres (`CSI P`) l'a DECALE avec sa
///      cellule : il n'y a plus rien a enrouler, le fanion est inerte ;
///    - sur la derniere colonne de la TOUTE DERNIERE ligne redessinee : il faudrait une
///      ligne suivante pour declencher l'enroulement. Cet etat n'existe que si la region de
///      defilement s'arrete avant la derniere ligne, sinon l'enroulement fait defiler ;
///    - sur la ligne ou le curseur se trouve EN BUTEE : reposer la butee demande de reecrire
///      cette cellule, ce qui efface son fanion (voir `retouche_butee`) ;
///    - sur une derniere colonne qui contient une TABULATION : `put_tab` n'avance pas quand
///      il est deja en derniere colonne, donc il ne met pas le curseur en butee et
///      l'enroulement ne peut plus etre declenche. Entre la tabulation et le fanion, le
///      redessin garde la tabulation : c'est elle qui porte du contenu.
/// 3. **La cellule qui SUIT un caractere large.** Une suppression de caracteres peut retirer
///    le remplissage d'un caractere large et laisser du contenu a sa place : la grille dit
///    alors qu'un glyphe de deux colonnes n'en occupe qu'une. Le redessin doit choisir :
///    reecrire le glyphe (et perdre la cellule d'a cote, que son remplissage recouvre) ou le
///    sauter (et perdre le glyphe). Il garde le GLYPHE, parce que c'est ce que l'utilisateur
///    voit — les deux colonnes etaient de toute facon occupees par lui a l'ecran.
/// 4. **Un caractere large en DERNIERE colonne.** Il ne peut pas y naitre : l'emulateur y
///    pose un remplissage et renvoie le glyphe a la ligne suivante. En trouver un la veut
///    dire qu'une suppression l'y a decale. Le reecrire ferait enrouler la ligne et
///    decalerait tout le reste du redessin, donc on le laisse tomber.
///
/// La mise de cote est FAITE SUR LES DEUX ETATS A LA FOIS, cellule par cellule : c'est la
/// seule facon d'etre symetrique quand la cause n'est visible que d'un cote (un fanion de
/// remplissage present dans la source et absent du redessin, par exemple).
///
/// Aucun programme reel ne produit ces etats : les traces de vim, htop, less, git log, ls
/// et claude passent l'aller-retour EXACT, et c'est `verifier` qui le verrouille.
fn mettre_de_cote_les_cas_connus(attendu: &mut EtatEcran, obtenu: &mut EtatEcran) {
    use alacritty_terminal::term::cell::Flags;
    const INDICES: Flags = Flags::WIDE_CHAR
        .union(Flags::WIDE_CHAR_SPACER)
        .union(Flags::LEADING_WIDE_CHAR_SPACER);

    let en_butee = attendu.curseur.attend_retour || obtenu.curseur.attend_retour;
    let ligne_du_curseur = attendu.curseur.ligne;
    let hauteur = attendu.lignes;
    let vierge = cellule_vierge();

    for principale in [true, false] {
        let (ga, go) = if principale {
            (&mut attendu.principale, &mut obtenu.principale)
        } else {
            (&mut attendu.alternative, &mut obtenu.alternative)
        };
        if ga.len() != go.len() {
            continue;
        }
        let derniere_ligne = ga.len().saturating_sub(1);
        // La grille peut porter de l'historique devant elle : la ligne 0 du CURSEUR n'est
        // pas la ligne 0 de la grille.
        let ligne_curseur = ga.len().saturating_sub(hauteur) as i32 + ligne_du_curseur;

        for (i, (la, lo)) in ga.iter_mut().zip(go.iter_mut()).enumerate() {
            if la.len() != lo.len() {
                continue;
            }
            let derniere_colonne = la.len().saturating_sub(1);
            let apres_un_large: Vec<bool> = (0..la.len())
                .map(|j| {
                    j > 0
                        && (la[j - 1].fanions.contains(Flags::WIDE_CHAR)
                            || lo[j - 1].fanions.contains(Flags::WIDE_CHAR))
                })
                .collect();

            for j in 0..la.len() {
                let (a, o) = (&mut la[j], &mut lo[j]);

                // 1. Indices de largeur orphelins sur des cellules vides.
                let un_indice = a.fanions.intersects(INDICES) || o.fanions.intersects(INDICES);
                let vides = matches!(a.c, ' ' | '\t') && matches!(o.c, ' ' | '\t');
                if un_indice && vides {
                    a.c = ' ';
                    o.c = ' ';
                    a.fanions.remove(INDICES);
                    o.fanions.remove(INDICES);
                }

                // 2. `WRAPLINE` inerte ou impossible a reposer.
                if j != derniere_colonne
                    || i == derniere_ligne
                    || a.c == '\t'
                    || o.c == '\t'
                    || (en_butee && i as i32 == ligne_curseur)
                {
                    a.fanions.remove(Flags::WRAPLINE);
                    o.fanions.remove(Flags::WRAPLINE);
                }

                // 3 et 4. Cellules qu'un caractere large rend irreproductibles : mises de
                // cote ENTIEREMENT, le redessin n'y pose rien du tout.
                let large_en_bout = j == derniere_colonne
                    && (a.fanions.contains(Flags::WIDE_CHAR)
                        || o.fanions.contains(Flags::WIDE_CHAR));
                if apres_un_large[j] || large_en_bout {
                    *a = vierge.clone();
                    *o = vierge.clone();
                }
            }
        }
    }
}

/// La cellule d'un ecran vierge, telle qu'un emulateur neuf la donne.
fn cellule_vierge() -> Cellule {
    ecran_avale(b"").etat().principale[0][0].clone()
}

/// Graines des tirages au hasard, et nombre de tirages par graine.
///
/// FIXES a dessein : un echec doit etre REJOUABLE. Un `rand::thread_rng` donnerait un
/// echec different a chaque execution, donc impossible a reduire. Le nombre est un
/// compromis — la campagne qui a servi a mettre au point le redessin a tourne a
/// 6 000 tirages par graine sur cinq jeux de graines differents (voir
/// `docs/portabilite/plan-terminaux.md`) ; ce qui reste ici est ce qu'on peut payer a
/// chaque `cargo test`.
const GRAINES: &[u64] = &[
    0x5EED_1234_ABCD_0051,
    0xDEAD_0000_1111_2277,
    0x0123_4567_89AB_CDE5,
    0xFFFF_1234_5678_9AB5,
];
const TIRAGES: usize = 400;

/// Generateur deterministe (xorshift64*). Volontairement maison : une graine fixe rend un
/// echec REJOUABLE, ce qu'un `rand::thread_rng` ne donne pas, et ca evite une dependance.
struct Hasard(u64);

impl Hasard {
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn borne(&mut self, max: usize) -> usize {
        (self.suivant() % max as u64) as usize
    }
}

#[test]
fn aller_retour_sur_des_octets_au_hasard() {
    let mut toleres = 0;
    for graine in GRAINES {
        let mut hasard = Hasard(*graine);
        for tirage in 0..TIRAGES {
            let taille = 1 + hasard.borne(3000);
            let octets: Vec<u8> =
                (0..taille).map(|_| (hasard.suivant() & 0xff) as u8).collect();
            if verifier_tolerant(&format!("octets au hasard #{graine:x}/{tirage}"), &octets) {
                toleres += 1;
            }
        }
    }
    eprintln!(
        "octets au hasard : {toleres} tirages sur {} mis de cote",
        GRAINES.len() * TIRAGES
    );
}

/// Des octets au hasard tires d'un alphabet de terminal : bien plus de sequences valides
/// qu'un tirage uniforme, donc bien plus d'etats interessants atteints.
#[test]
fn aller_retour_sur_des_sequences_au_hasard() {
    const MORCEAUX: &[&str] = &[
        "\x1b[", "\x1b]", "\x1b", "[", "]", ";", "?", "0", "1", "2", "3", "4", "5", "7", "8", "9",
        "12", "24", "25", "38", "48", "49", "58", "1000", "1002", "1003", "1049", "2004", "m", "H",
        "J", "K", "r", "h", "l", "q", "X", "P", "@", "L", "M", "S", "T", "d", "G", "A", "B", "C",
        "D", "t", "n", "\x07", "\r", "\n", "\t", "\x08", "\x0e", "\x0f", "a", "Z", " ", "(0", "(B",
        "7", "8", "c", "M", "D", "E", "H", "abc", "texte long qui deborde peut etre",
        "日本語", "e\u{301}", "🚀", "\u{20D7}", "\x1b[?", "\x1b[1;1", "\x1b[10;20",
    ];
    let mut toleres = 0;
    for graine in GRAINES {
        let mut hasard = Hasard(graine.rotate_left(17));
        for tirage in 0..TIRAGES {
            let morceaux = 1 + hasard.borne(400);
            let mut octets = Vec::new();
            for _ in 0..morceaux {
                octets.extend_from_slice(MORCEAUX[hasard.borne(MORCEAUX.len())].as_bytes());
            }
            if verifier_tolerant(&format!("sequences au hasard #{graine:x}/{tirage}"), &octets) {
                toleres += 1;
            }
        }
    }
    eprintln!(
        "sequences au hasard : {toleres} tirages sur {} mis de cote",
        GRAINES.len() * TIRAGES
    );
}

/// Les memes octets, mais livres en morceaux de taille aleatoire : c'est ce que fait un
/// PTY. Une sequence coupee en deux ne doit pas changer l'etat final.
#[test]
fn le_decoupage_du_flux_ne_change_rien() {
    let mut hasard = Hasard(0x1357_9BDF_0246_8ACE);
    for (nom, octets) in traces() {
        let entier = ecran_avale(octets).etat();
        let mut morcele = Ecran::avec_historique(COLONNES, LIGNES, HISTORIQUE);
        let mut reste = octets;
        while !reste.is_empty() {
            let taille = (1 + hasard.borne(97)).min(reste.len());
            morcele.avaler(&reste[..taille]);
            reste = &reste[taille..];
        }
        let obtenu = morcele.etat();
        if let Some(quoi) = entier.difference(&obtenu) {
            panic!("le decoupage change l'etat sur « {nom} » : {quoi}");
        }
    }
}

// --- 3. Vraies traces ---

#[test]
fn aller_retour_sur_de_vraies_traces() {
    for (nom, octets) in traces() {
        verifier(nom, octets);
    }
}

/// Les traces coupees a une longueur arbitraire : un etat de milieu de dessin, souvent
/// plus tordu que l'etat final (sequence a moitie ecrite, ecran a moitie repeint).
#[test]
fn aller_retour_sur_des_traces_tronquees() {
    for (nom, octets) in traces() {
        let mut coupe = 1;
        while coupe < octets.len() {
            verifier(&format!("{nom} tronque a {coupe}"), &octets[..coupe]);
            coupe = coupe * 3 / 2 + 7;
        }
    }
}

// --- Proprietes qui ne se lisent pas dans l'aller-retour ---

#[test]
fn la_principale_survit_a_l_ecran_alternatif() {
    let mut ecran = ecran_avale(b"contenu de la principale\r\ndeuxieme ligne");
    let avant = ecran.etat();
    ecran.avaler(b"\x1b[?1049h\x1b[2Jplein ecran");
    assert!(ecran.ecran_alternatif(), "l'ecran alternatif doit etre actif");
    ecran.avaler(b"\x1b[?1049l");
    assert!(!ecran.ecran_alternatif());
    let apres = ecran.etat();
    assert_eq!(
        avant.principale, apres.principale,
        "la grille principale doit etre intacte au retour de l'ecran alternatif"
    );
}

#[test]
fn le_redessin_ne_transmet_pas_la_rafale() {
    // Une grosse sortie qui defile : ce qui remonte doit etre ce que l'ecran MONTRE, pas
    // tout ce que le shell a ecrit. C'est le service que tmux rendait sans qu'on le sache.
    let rafale = rafale(4 * 1024 * 1024);
    let ecran = ecran_avale(&rafale);
    let ecran_seul = ecran.redessiner_ecran().len();
    let avec_historique = ecran.redessiner().len();
    assert!(
        ecran_seul < 8 * 1024,
        "un ecran de 80x24 ne doit pas couter plus de 8 Ko a redessiner, mesure {ecran_seul}"
    );
    assert!(
        avec_historique < rafale.len() / 4,
        "redessiner 10 000 lignes d'historique doit couter tres largement moins que la \
         rafale ({avec_historique} contre {})",
        rafale.len()
    );
    eprintln!(
        "redessin : ecran seul {ecran_seul} octets, avec historique {avec_historique} octets, \
         pour une rafale de {} octets",
        rafale.len()
    );
}

#[test]
fn les_reponses_au_shell_sont_ramassees() {
    // CPR (position du curseur) et DA (identification) : un programme qui les envoie
    // ATTEND la reponse. Si elle est perdue, il se fige — d'ou la file de sortants.
    let mut ecran = ecran_avale(b"\x1b[3;5H\x1b[6n");
    let sortants = ecran.sortants();
    assert_eq!(
        sortants,
        vec![Sortant::VersLeShell("\x1b[3;5R".to_string())],
        "la position du curseur doit remonter"
    );
    assert!(ecran.sortants().is_empty(), "la file doit se vider en la lisant");

    // OSC 52 : la copie vers le presse-papier systeme.
    ecran.avaler(b"\x1b]52;c;Y29weQ==\x07");
    assert_eq!(
        ecran.sortants(),
        vec![Sortant::VersLePressePapier("copy".to_string())]
    );
}

#[test]
fn le_redimensionnement_remet_la_region_a_l_ecran_entier() {
    let mut ecran = ecran_avale(b"\x1b[5;20rtexte");
    assert_eq!(ecran.etat().region, (4, 20));
    ecran.redimensionner(100, 40);
    assert_eq!(
        ecran.etat().region,
        (0, 40),
        "comme alacritty, un redimensionnement rend la region a l'ecran entier"
    );
    verifier("apres redimensionnement", b"");
}

#[test]
fn une_taille_absurde_ne_fait_pas_paniquer() {
    // Un conteneur pas encore mesure envoie 0x0. Silence VOLONTAIRE : ce n'est pas une
    // action de l'utilisateur, il n'y a rien a lui notifier.
    let mut ecran = Ecran::nouveau(0, 0);
    ecran.avaler(b"texte");
    ecran.redimensionner(0, 0);
    assert!(ecran.colonnes() >= 2 && ecran.lignes() >= 1);
}

// --- Performance : des chiffres, pas des impressions ---

/// Une grosse sortie realiste : des lignes de longueur variable, avec des couleurs.
fn rafale(taille: usize) -> Vec<u8> {
    let mut hasard = Hasard(0x0BAD_F00D_DEAD_BEEF);
    let mut octets = Vec::with_capacity(taille + 128);
    let mut i = 0u64;
    while octets.len() < taille {
        i += 1;
        let largeur = 20 + hasard.borne(60);
        if i % 7 == 0 {
            octets.extend_from_slice(format!("\x1b[38;5;{}m", i % 256).as_bytes());
        }
        octets.extend_from_slice(format!("{i:>8} ").as_bytes());
        for _ in 0..largeur {
            octets.push(b'a' + (hasard.borne(26)) as u8);
        }
        octets.extend_from_slice(b"\x1b[m\r\n");
    }
    octets
}

#[test]
fn cout_ingestion_rafale() {
    let rafale = rafale(4 * 1024 * 1024);
    let mut ecran = Ecran::nouveau(COLONNES, LIGNES);
    let debut = std::time::Instant::now();
    // Par morceaux de 64 Ko, comme un PTY les livre.
    for morceau in rafale.chunks(64 * 1024) {
        ecran.avaler(morceau);
    }
    let duree = debut.elapsed();
    eprintln!(
        "ingestion de {} Mo : {:?} ({:.1} Mo/s)",
        rafale.len() / (1024 * 1024),
        duree,
        rafale.len() as f64 / duree.as_secs_f64() / (1024.0 * 1024.0)
    );
    // Borne large a dessein : ce test tourne aussi en debug, ou l'emulateur est dix fois
    // plus lent. Le chiffre qui compte est celui qu'affiche `--nocapture` en release.
    assert!(
        duree.as_secs_f64() < 20.0,
        "ingestion de 4 Mo en {duree:?} : quelque chose s'est effondre"
    );
}

#[test]
fn cout_serialisation() {
    let ecran = ecran_avale(&rafale(4 * 1024 * 1024));
    let debut = std::time::Instant::now();
    let mut octets = 0;
    for _ in 0..100 {
        octets = ecran.redessiner_ecran().len();
    }
    let par_appel = debut.elapsed() / 100;
    eprintln!("redessin d'un ecran seul : {par_appel:?} pour {octets} octets");

    let debut = std::time::Instant::now();
    let mut octets = 0;
    for _ in 0..10 {
        octets = ecran.redessiner().len();
    }
    let par_appel = debut.elapsed() / 10;
    eprintln!("redessin ecran + 10 000 lignes d'historique : {par_appel:?} pour {octets} octets");
    assert!(par_appel.as_secs_f64() < 5.0);
}

#[test]
fn cout_memoire_historique_plein() {
    // Les octets sont fabriques ligne par ligne, pas dans un grand tampon : la mesure doit
    // porter sur l'emulateur, pas sur le tampon d'entree.
    // On mesure le cout d'une session SUPPLEMENTAIRE : la premiere paie aussi les tampons
    // de l'analyseur et le bruit du harnais de test, la seconde donne le prix marginal —
    // c'est lui qui compte quand un utilisateur ouvre dix terminaux.
    let _premiere = session_pleine();
    let avant = memoire_resident();
    let ecran = session_pleine();
    let apres = memoire_resident();
    assert_eq!(
        ecran.etat().principale.len(),
        HISTORIQUE + LIGNES,
        "l'historique doit etre plein"
    );
    match (avant, apres) {
        (Some(a), Some(b)) => eprintln!(
            "memoire d'une session SUPPLEMENTAIRE avec {HISTORIQUE} lignes d'historique : {} Ko \
             ({} octets par cellule x {} cellules = {} Ko de grille)",
            b.saturating_sub(a) / 1024,
            std::mem::size_of::<alacritty_terminal::term::cell::Cell>(),
            (HISTORIQUE + LIGNES) * COLONNES,
            std::mem::size_of::<alacritty_terminal::term::cell::Cell>()
                * (HISTORIQUE + LIGNES)
                * COLONNES
                / 1024
        ),
        _ => eprintln!("memoire resident non lisible sur ce systeme"),
    }
}

/// Une session dont l'historique de 10 000 lignes est PLEIN.
fn session_pleine() -> Ecran {
    let mut ecran = Ecran::avec_historique(COLONNES, LIGNES, HISTORIQUE);
    for i in 0..(HISTORIQUE + LIGNES) {
        ecran.avaler(
            format!("ligne {i} avec du texte pour occuper la largeur de l'ecran\r\n").as_bytes(),
        );
    }
    ecran
}

/// Memoire resident du processus, en octets. `None` hors Linux.
fn memoire_resident() -> Option<usize> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: usize = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}

/// Verrouille la forme de la photo d'etat : si une de ces valeurs change de sens, le test
/// d'aller-retour continuerait de passer en comparant moins de choses.
#[test]
fn la_photo_d_etat_decrit_bien_l_ecran() {
    let ecran = ecran_avale(b"\x1b[5;20r\x1b[?25l\x1b[1;41mA\x1b]2;titre\x07");
    let etat: EtatEcran = ecran.etat();
    assert_eq!(etat.colonnes, COLONNES);
    assert_eq!(etat.lignes, LIGNES);
    assert!(!etat.alternatif);
    assert_eq!(etat.principale.len(), LIGNES, "pas d'historique, pas de defilement");
    assert_eq!(etat.principale[0][0].c, 'A');
    assert_eq!(etat.region, (4, 20));
    assert_eq!(etat.titre.as_deref(), Some("titre"));
    assert_eq!(etat.curseur.ligne, 0);
    assert_eq!(etat.curseur.colonne, 1);
}

