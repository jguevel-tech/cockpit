//! Ce qui circule dans le tuyau entre l'application et le service de terminaux.
//!
//! ## La poignee de main, et pourquoi c'est le SERVICE qui parle en premier
//!
//! Le service survit a l'application : une application neuve parlera un jour a un service
//! ancien, et l'inverse arrivera aussi (l'utilisateur revient a une version precedente).
//! Si la version voyageait dans un message ordinaire, la partie la plus recente devrait
//! comprendre le format de la plus ancienne pour la lire — c'est exactement le piege du
//! « protocol version mismatch » de tmux, ou l'on ne sait dire QUE que ca ne marche pas.
//!
//! Donc : des qu'une connexion s'ouvre, le service envoie un preambule de taille fixe —
//! `CKPTERM\0` puis sa version sur deux octets, gros-boutiste. **Ces dix octets ne
//! changeront JAMAIS de forme**, quelle que soit la suite du protocole. Le client les lit
//! avant d'envoyer quoi que ce soit et sait immediatement s'il a en face un service plus
//! ancien, plus recent, ou pas un service du tout. Il en rend une erreur STRUCTUREE
//! (`ErreurPoignee`), avec les deux numeros — pas une chaine a analyser.
//!
//! ## Le format des messages
//!
//! Une trame = sa longueur sur 4 octets gros-boutiste, puis le corps. Le corps commence
//! par un octet de genre (requete / reponse / poussee), puis un octet de variante, puis
//! les champs a la suite. Encodage ecrit a la main plutot que JSON pour deux raisons : la
//! sortie d'un terminal est BINAIRE (JSON imposerait du base64, +33 % sur le chemin le
//! plus charge) et le chemin de frappe doit rester une ecriture directe, sans passer par
//! un serialiseur generique.
//!
//! Ajouter un champ a une variante existante = casser le format = **incrementer
//! `VERSION`**. Ajouter une variante a la fin d'une enumeration aussi : un service ancien
//! rendrait « genre inconnu » sur un message qu'il n'a jamais connu.

use std::io::{self, Read, Write};

/// Version du protocole. A INCREMENTER des qu'un message change de forme.
///
/// **2** : `Creer` porte un ecran initial, et `Instantane` permet de photographier un
/// terminal. C'est ce qui rend les terminaux retrouvables apres l'extinction du poste.
///
/// **UN CHANGEMENT DE VERSION TUE LES SESSIONS EN COURS, UNE FOIS.** L'application refuse
/// de parler a un service d'une autre version (voir `comparer_versions`), et le service
/// vivant ne se remplace pas tout seul : c'est `adaptateur` qui lui demande de s'arreter
/// (`Arreter`, variante inchangee depuis la v1, donc comprise par un service ancien) avant
/// d'en lancer un neuf. Sans cette demande, l'application resterait bloquee sur
/// « service trop ancien » jusqu'au prochain redemarrage de la machine.
pub const VERSION: u16 = 2;

/// Les huit octets qui identifient le service. Jamais de changement de forme.
const MAGIE: [u8; 8] = *b"CKPTERM\0";

/// Taille du preambule : la magie plus deux octets de version.
pub const TAILLE_PREAMBULE: usize = MAGIE.len() + 2;

/// Plafond d'une trame. Une trame plus grosse est un flux desynchronise ou un
/// interlocuteur qui n'est pas le notre : on refuse au lieu d'allouer.
const TRAME_MAX: usize = 64 * 1024 * 1024;

/// Ce qui peut mal se passer AVANT que la conversation commence.
#[derive(Debug, PartialEq, Eq)]
pub enum ErreurPoignee {
    /// Quelque chose ecoute, mais ce n'est pas le service de terminaux de Cockpit.
    PasNotreService,
    /// Un service d'une version PLUS ANCIENNE tourne encore (terminaux ouverts avant la
    /// mise a jour). Il ne comprendra pas nos messages.
    ServiceTropAncien { service: u16, application: u16 },
    /// Le service est plus RECENT que nous : l'utilisateur est revenu a une version
    /// precedente de Cockpit sans fermer ses terminaux.
    ApplicationTropAncienne { service: u16, application: u16 },
    /// La connexion s'est coupee, ou le socket n'appartient pas a l'utilisateur.
    Tuyau(String),
}

impl std::fmt::Display for ErreurPoignee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PasNotreService => {
                write!(f, "ce n'est pas le service de terminaux de Cockpit qui repond sur ce socket")
            }
            Self::ServiceTropAncien { service, application } => write!(
                f,
                "un service de terminaux plus ancien tourne encore (protocole {service}, \
                 celui-ci parle le {application}) ; ferme les terminaux ouverts pour qu'il \
                 s'arrete, ils repartiront sur le nouveau"
            ),
            Self::ApplicationTropAncienne { service, application } => write!(
                f,
                "le service de terminaux parle le protocole {service}, cette version de \
                 Cockpit le {application} : c'est une version plus recente de Cockpit qui \
                 l'a demarre"
            ),
            Self::Tuyau(detail) => write!(f, "connexion au service de terminaux : {detail}"),
        }
    }
}

impl From<io::Error> for ErreurPoignee {
    fn from(e: io::Error) -> Self {
        Self::Tuyau(e.to_string())
    }
}

/// Envoie le preambule. Premier geste du service sur toute connexion acceptee.
pub fn ecrire_preambule(sortie: &mut impl Write) -> io::Result<()> {
    let mut tampon = [0u8; TAILLE_PREAMBULE];
    tampon[..MAGIE.len()].copy_from_slice(&MAGIE);
    tampon[MAGIE.len()..].copy_from_slice(&VERSION.to_be_bytes());
    sortie.write_all(&tampon)?;
    sortie.flush()
}

/// Lit le preambule et tranche : peut-on se parler ?
pub fn lire_preambule(entree: &mut impl Read) -> Result<(), ErreurPoignee> {
    let mut tampon = [0u8; TAILLE_PREAMBULE];
    entree.read_exact(&mut tampon)?;
    if tampon[..MAGIE.len()] != MAGIE {
        return Err(ErreurPoignee::PasNotreService);
    }
    let service = u16::from_be_bytes([tampon[MAGIE.len()], tampon[MAGIE.len() + 1]]);
    comparer_versions(service, VERSION)
}

/// La regle de compatibilite, isolee pour etre testable sans socket.
///
/// Egalite STRICTE : deux versions differentes ne se parlent pas. Une compatibilite
/// partielle (« le service ancien comprend au moins creer et ecrire ») donnerait une
/// application a moitie fonctionnelle, ce qui est pire qu'un message clair.
pub fn comparer_versions(service: u16, application: u16) -> Result<(), ErreurPoignee> {
    match service.cmp(&application) {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Less => Err(ErreurPoignee::ServiceTropAncien { service, application }),
        std::cmp::Ordering::Greater => {
            Err(ErreurPoignee::ApplicationTropAncienne { service, application })
        }
    }
}

/// Taille de la grille, en cellules.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Taille {
    pub colonnes: u16,
    pub lignes: u16,
}

/// Une cellule designee par le frontend (selection a la souris).
/// `ligne` suit la convention d'alacritty : 0 = premiere ligne visible, negatif = historique.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Position {
    pub ligne: i32,
    pub colonne: u16,
}

/// Ce que le service sait d'un terminal. Le NOM et le PROJET n'y sont pas : ils vivent en
/// SQLite, qui survit au redemarrage de la machine alors que le service, non.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InfoSession {
    pub id: i64,
    /// Le shell tourne encore.
    pub vivant: bool,
    /// Un CLI d'agent LLM (claude, codex...) tourne dans la session.
    pub llm: bool,
    pub taille: Taille,
}

/// Les gestes de la recherche, tels que la barre d'onglets les declenche.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionRecherche {
    Demarrer,
    Suivante,
    Precedente,
    Annuler,
}

/// Ce que l'application demande au service. Vient du trait de l'etape A, et de lui seul.
///
/// N'y figurent PAS : `detacher` et `ecran_alternatif` (contournements de tmux sans
/// appelant), et `renommer` — le nom d'onglet vit en SQLite, le mettre aussi dans le
/// service ferait deux verites pour une seule chaine.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Requete {
    /// L'identifiant vient de l'APPLICATION (rowid SQLite) : il doit survivre au
    /// redemarrage de la machine, ce que le service ne fait pas.
    Creer {
        id: i64,
        dossier: String,
        taille: Taille,
        commande_initiale: Option<String>,
        /// Ce que le terminal affichait la derniere fois, a remettre dans l'ecran AVANT que
        /// le shell neuf n'ecrive quoi que ce soit. Vide pour un terminal qui nait.
        ///
        /// Passe par le SERVICE et non par le terminal du frontend, pour une raison d'ordre :
        /// l'attache qui suit une creation pousse un redessin, et un redessin commence par
        /// une remise a plat qui viderait un contenu ecrit par le frontend. Injecte ici, le
        /// contenu traverse l'emulateur : il se retrouve dans l'historique, donc dans le
        /// redessin, la molette et la recherche.
        ecran_initial: Vec<u8>,
    },
    /// Le chemin de frappe. Aucune reponse n'est attendue : un aller-retour ajouterait la
    /// latence du tuyau a CHAQUE touche. Un echec revient en `Pousse::Panne`.
    Ecrire { id: i64, octets: Vec<u8> },
    Redimensionner { id: i64, taille: Taille },
    Fermer { id: i64 },
    /// Branche cette connexion sur la sortie du terminal. Ne rend rien : le redessin
    /// arrive par le meme canal que la suite (`Pousse::Redessin`).
    Attacher { id: i64, taille: Taille },
    Lister,
    Chercher { id: i64, action: ActionRecherche, motif: String },
    /// Le texte d'une region, bornes comprises (clic droit > Copier).
    CopierSelection { id: i64, debut: Position, fin: Position },
    /// Redemande l'etat complet, par exemple pour remplir l'historique de defilement.
    Redessiner { id: i64, avec_historique: bool },
    /// Arret du service. Les shells meurent avec lui.
    Arreter,
    /// Photographie ce terminal : les octets qui le redessineraient tel quel.
    ///
    /// Rendus par le service et non calcules par le frontend : l'emulateur du service est
    /// l'autorite sur ce qu'affiche un terminal, et une serialisation cote interface
    /// couterait du temps sur le fil qui dessine, a chaque terminal.
    Instantane { id: i64 },
}

/// La reponse a une requete, portant le meme numero de sequence.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Reponse {
    Fait,
    Erreur(String),
    Sessions(Vec<InfoSession>),
    Texte(String),
    /// `index` est la position de l'occurrence courante dans `total`, `None` si aucune.
    Recherche { total: u32, index: Option<u32>, occurrence: Option<Position> },
    /// Les octets qui redessinent un terminal tel qu'il est (reponse a `Instantane`).
    Octets(Vec<u8>),
}

/// Ce que le service envoie sans qu'on le lui demande.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Pousse {
    /// Suite d'octets a donner telle quelle au terminal du frontend.
    Sortie { id: i64, octets: Vec<u8> },
    /// Etat COMPLET de l'ecran : a appliquer par-dessus n'importe quel etat, la suite
    /// commence par une remise a plat.
    Redessin { id: i64, octets: Vec<u8> },
    /// OSC 52 : le programme demande a poser du texte dans le presse-papier systeme.
    PressePapier { id: i64, texte: String },
    /// Le shell s'est termine.
    Fini { id: i64 },
    /// Panne sur ce terminal, hors de toute requete (echec d'ecriture, PTY perdu).
    Panne { id: i64, message: String },
}

/// Une trame complete, dans un sens ou dans l'autre.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Trame {
    Requete { sequence: u32, requete: Requete },
    Reponse { sequence: u32, reponse: Reponse },
    Pousse(Pousse),
}

// --- Encodage ---

impl Trame {
    pub fn encoder(&self) -> Vec<u8> {
        let mut corps = Vec::with_capacity(64);
        match self {
            Self::Requete { sequence, requete } => {
                corps.push(1);
                corps.extend_from_slice(&sequence.to_be_bytes());
                encoder_requete(requete, &mut corps);
            }
            Self::Reponse { sequence, reponse } => {
                corps.push(2);
                corps.extend_from_slice(&sequence.to_be_bytes());
                encoder_reponse(reponse, &mut corps);
            }
            Self::Pousse(pousse) => {
                corps.push(3);
                encoder_pousse(pousse, &mut corps);
            }
        }
        let mut trame = Vec::with_capacity(4 + corps.len());
        trame.extend_from_slice(&(corps.len() as u32).to_be_bytes());
        trame.extend_from_slice(&corps);
        trame
    }

    pub fn decoder(corps: &[u8]) -> io::Result<Self> {
        let mut l = Lecteur::neuf(corps);
        let trame = match l.octet()? {
            1 => Self::Requete { sequence: l.u32()?, requete: decoder_requete(&mut l)? },
            2 => Self::Reponse { sequence: l.u32()?, reponse: decoder_reponse(&mut l)? },
            3 => Self::Pousse(decoder_pousse(&mut l)?),
            autre => return Err(abime(format!("genre de trame inconnu: {autre}"))),
        };
        // Des octets en trop = les deux cotes ne parlent pas le meme protocole, meme si le
        // numero de version dit l'inverse. Mieux vaut le dire que travailler sur du sable.
        if !l.fini() {
            return Err(abime("trame plus longue que son contenu".into()));
        }
        Ok(trame)
    }

    /// Lit une trame sur un flux. `Ok(None)` = l'autre bout a ferme proprement.
    pub fn lire(entree: &mut impl Read) -> io::Result<Option<Self>> {
        let mut entete = [0u8; 4];
        match entree.read_exact(&mut entete) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }
        let taille = u32::from_be_bytes(entete) as usize;
        if taille > TRAME_MAX {
            return Err(abime(format!("trame de {taille} octets, plafond {TRAME_MAX}")));
        }
        let mut corps = vec![0u8; taille];
        entree.read_exact(&mut corps)?;
        Self::decoder(&corps).map(Some)
    }
}

fn abime(detail: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, detail)
}

fn encoder_requete(r: &Requete, out: &mut Vec<u8>) {
    match r {
        Requete::Creer { id, dossier, taille, commande_initiale, ecran_initial } => {
            out.push(1);
            mettre_i64(*id, out);
            mettre_chaine(dossier, out);
            mettre_taille(*taille, out);
            mettre_option(commande_initiale.as_deref(), out);
            mettre_octets(ecran_initial, out);
        }
        Requete::Ecrire { id, octets } => {
            out.push(2);
            mettre_i64(*id, out);
            mettre_octets(octets, out);
        }
        Requete::Redimensionner { id, taille } => {
            out.push(3);
            mettre_i64(*id, out);
            mettre_taille(*taille, out);
        }
        Requete::Fermer { id } => {
            out.push(4);
            mettre_i64(*id, out);
        }
        Requete::Attacher { id, taille } => {
            out.push(5);
            mettre_i64(*id, out);
            mettre_taille(*taille, out);
        }
        Requete::Lister => out.push(6),
        Requete::Chercher { id, action, motif } => {
            out.push(7);
            mettre_i64(*id, out);
            out.push(match action {
                ActionRecherche::Demarrer => 0,
                ActionRecherche::Suivante => 1,
                ActionRecherche::Precedente => 2,
                ActionRecherche::Annuler => 3,
            });
            mettre_chaine(motif, out);
        }
        Requete::CopierSelection { id, debut, fin } => {
            out.push(8);
            mettre_i64(*id, out);
            mettre_position(*debut, out);
            mettre_position(*fin, out);
        }
        Requete::Redessiner { id, avec_historique } => {
            out.push(9);
            mettre_i64(*id, out);
            out.push(*avec_historique as u8);
        }
        Requete::Arreter => out.push(10),
        Requete::Instantane { id } => {
            out.push(11);
            mettre_i64(*id, out);
        }
    }
}

fn decoder_requete(l: &mut Lecteur) -> io::Result<Requete> {
    Ok(match l.octet()? {
        1 => Requete::Creer {
            id: l.i64()?,
            dossier: l.chaine()?,
            taille: l.taille()?,
            commande_initiale: l.option()?,
            ecran_initial: l.octets()?,
        },
        2 => Requete::Ecrire { id: l.i64()?, octets: l.octets()? },
        3 => Requete::Redimensionner { id: l.i64()?, taille: l.taille()? },
        4 => Requete::Fermer { id: l.i64()? },
        5 => Requete::Attacher { id: l.i64()?, taille: l.taille()? },
        6 => Requete::Lister,
        7 => Requete::Chercher {
            id: l.i64()?,
            action: match l.octet()? {
                0 => ActionRecherche::Demarrer,
                1 => ActionRecherche::Suivante,
                2 => ActionRecherche::Precedente,
                3 => ActionRecherche::Annuler,
                autre => return Err(abime(format!("action de recherche inconnue: {autre}"))),
            },
            motif: l.chaine()?,
        },
        8 => Requete::CopierSelection { id: l.i64()?, debut: l.position()?, fin: l.position()? },
        9 => Requete::Redessiner { id: l.i64()?, avec_historique: l.booleen()? },
        10 => Requete::Arreter,
        11 => Requete::Instantane { id: l.i64()? },
        autre => return Err(abime(format!("requete inconnue: {autre}"))),
    })
}

fn encoder_reponse(r: &Reponse, out: &mut Vec<u8>) {
    match r {
        Reponse::Fait => out.push(1),
        Reponse::Erreur(message) => {
            out.push(2);
            mettre_chaine(message, out);
        }
        Reponse::Sessions(sessions) => {
            out.push(3);
            out.extend_from_slice(&(sessions.len() as u32).to_be_bytes());
            for s in sessions {
                mettre_i64(s.id, out);
                out.push(s.vivant as u8);
                out.push(s.llm as u8);
                mettre_taille(s.taille, out);
            }
        }
        Reponse::Texte(texte) => {
            out.push(4);
            mettre_chaine(texte, out);
        }
        Reponse::Recherche { total, index, occurrence } => {
            out.push(5);
            out.extend_from_slice(&total.to_be_bytes());
            match index {
                Some(i) => {
                    out.push(1);
                    out.extend_from_slice(&i.to_be_bytes());
                }
                None => out.push(0),
            }
            match occurrence {
                Some(p) => {
                    out.push(1);
                    mettre_position(*p, out);
                }
                None => out.push(0),
            }
        }
        Reponse::Octets(octets) => {
            out.push(6);
            mettre_octets(octets, out);
        }
    }
}

fn decoder_reponse(l: &mut Lecteur) -> io::Result<Reponse> {
    Ok(match l.octet()? {
        1 => Reponse::Fait,
        2 => Reponse::Erreur(l.chaine()?),
        3 => {
            let combien = l.u32()? as usize;
            let mut sessions = Vec::with_capacity(combien.min(1024));
            for _ in 0..combien {
                sessions.push(InfoSession {
                    id: l.i64()?,
                    vivant: l.booleen()?,
                    llm: l.booleen()?,
                    taille: l.taille()?,
                });
            }
            Reponse::Sessions(sessions)
        }
        4 => Reponse::Texte(l.chaine()?),
        5 => Reponse::Recherche {
            total: l.u32()?,
            index: if l.booleen()? { Some(l.u32()?) } else { None },
            occurrence: if l.booleen()? { Some(l.position()?) } else { None },
        },
        6 => Reponse::Octets(l.octets()?),
        autre => return Err(abime(format!("reponse inconnue: {autre}"))),
    })
}

fn encoder_pousse(p: &Pousse, out: &mut Vec<u8>) {
    match p {
        Pousse::Sortie { id, octets } => {
            out.push(1);
            mettre_i64(*id, out);
            mettre_octets(octets, out);
        }
        Pousse::Redessin { id, octets } => {
            out.push(2);
            mettre_i64(*id, out);
            mettre_octets(octets, out);
        }
        Pousse::PressePapier { id, texte } => {
            out.push(3);
            mettre_i64(*id, out);
            mettre_chaine(texte, out);
        }
        Pousse::Fini { id } => {
            out.push(4);
            mettre_i64(*id, out);
        }
        Pousse::Panne { id, message } => {
            out.push(5);
            mettre_i64(*id, out);
            mettre_chaine(message, out);
        }
    }
}

fn decoder_pousse(l: &mut Lecteur) -> io::Result<Pousse> {
    Ok(match l.octet()? {
        1 => Pousse::Sortie { id: l.i64()?, octets: l.octets()? },
        2 => Pousse::Redessin { id: l.i64()?, octets: l.octets()? },
        3 => Pousse::PressePapier { id: l.i64()?, texte: l.chaine()? },
        4 => Pousse::Fini { id: l.i64()? },
        5 => Pousse::Panne { id: l.i64()?, message: l.chaine()? },
        autre => return Err(abime(format!("poussee inconnue: {autre}"))),
    })
}

fn mettre_i64(v: i64, out: &mut Vec<u8>) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn mettre_taille(t: Taille, out: &mut Vec<u8>) {
    out.extend_from_slice(&t.colonnes.to_be_bytes());
    out.extend_from_slice(&t.lignes.to_be_bytes());
}

fn mettre_position(p: Position, out: &mut Vec<u8>) {
    out.extend_from_slice(&p.ligne.to_be_bytes());
    out.extend_from_slice(&p.colonne.to_be_bytes());
}

fn mettre_octets(v: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&(v.len() as u32).to_be_bytes());
    out.extend_from_slice(v);
}

fn mettre_chaine(v: &str, out: &mut Vec<u8>) {
    mettre_octets(v.as_bytes(), out);
}

fn mettre_option(v: Option<&str>, out: &mut Vec<u8>) {
    match v {
        Some(s) => {
            out.push(1);
            mettre_chaine(s, out);
        }
        None => out.push(0),
    }
}

/// Lecture d'un corps de trame, champ par champ. Toute lecture au-dela de la fin est une
/// erreur REMONTEE, jamais une valeur par defaut : un champ tronque signifie que les deux
/// cotes ne s'entendent pas, et continuer produirait des identifiants inventes.
struct Lecteur<'a> {
    octets: &'a [u8],
    curseur: usize,
}

impl<'a> Lecteur<'a> {
    fn neuf(octets: &'a [u8]) -> Self {
        Self { octets, curseur: 0 }
    }

    fn fini(&self) -> bool {
        self.curseur == self.octets.len()
    }

    fn prendre(&mut self, combien: usize) -> io::Result<&'a [u8]> {
        let fin = self.curseur.checked_add(combien).ok_or_else(|| abime("champ demesure".into()))?;
        if fin > self.octets.len() {
            return Err(abime(format!(
                "trame tronquee : {combien} octets demandes, {} disponibles",
                self.octets.len() - self.curseur
            )));
        }
        let tranche = &self.octets[self.curseur..fin];
        self.curseur = fin;
        Ok(tranche)
    }

    fn octet(&mut self) -> io::Result<u8> {
        Ok(self.prendre(1)?[0])
    }

    fn booleen(&mut self) -> io::Result<bool> {
        Ok(self.octet()? != 0)
    }

    fn u16(&mut self) -> io::Result<u16> {
        let v = self.prendre(2)?;
        Ok(u16::from_be_bytes([v[0], v[1]]))
    }

    fn u32(&mut self) -> io::Result<u32> {
        let v = self.prendre(4)?;
        Ok(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }

    fn i32(&mut self) -> io::Result<i32> {
        Ok(self.u32()? as i32)
    }

    fn i64(&mut self) -> io::Result<i64> {
        let v = self.prendre(8)?;
        Ok(i64::from_be_bytes(v.try_into().expect("8 octets")))
    }

    fn taille(&mut self) -> io::Result<Taille> {
        Ok(Taille { colonnes: self.u16()?, lignes: self.u16()? })
    }

    fn position(&mut self) -> io::Result<Position> {
        Ok(Position { ligne: self.i32()?, colonne: self.u16()? })
    }

    fn octets(&mut self) -> io::Result<Vec<u8>> {
        let combien = self.u32()? as usize;
        Ok(self.prendre(combien)?.to_vec())
    }

    fn chaine(&mut self) -> io::Result<String> {
        let brut = self.octets()?;
        String::from_utf8(brut).map_err(|e| abime(format!("chaine non UTF-8: {e}")))
    }

    fn option(&mut self) -> io::Result<Option<String>> {
        if self.booleen()? {
            Ok(Some(self.chaine()?))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aller_retour(trame: Trame) {
        let encodee = trame.encoder();
        let relue = Trame::lire(&mut encodee.as_slice()).unwrap().unwrap();
        assert_eq!(relue, trame);
    }

    /// Chaque message doit ressortir IDENTIQUE. Un champ oublie a l'encodage ou lu dans le
    /// mauvais ordre ne se voit pas autrement qu'ici — a l'execution il donnerait un
    /// identifiant absurde, jamais une erreur.
    #[test]
    fn chaque_message_fait_l_aller_retour() {
        let taille = Taille { colonnes: 120, lignes: 40 };
        let position = Position { ligne: -42, colonne: 7 };
        for trame in [
            Trame::Requete {
                sequence: 1,
                requete: Requete::Creer {
                    id: 12,
                    dossier: "/home/moi/projet".into(),
                    taille,
                    commande_initiale: Some("npm run dev".into()),
                    ecran_initial: vec![27, b'[', b'H', 0, 255],
                },
            },
            Trame::Requete {
                sequence: 2,
                requete: Requete::Creer {
                    id: -1,
                    dossier: String::new(),
                    taille,
                    commande_initiale: None,
                    ecran_initial: Vec::new(),
                },
            },
            Trame::Requete { sequence: 3, requete: Requete::Ecrire { id: 4, octets: vec![0, 27, 255] } },
            Trame::Requete { sequence: 4, requete: Requete::Redimensionner { id: 4, taille } },
            Trame::Requete { sequence: 5, requete: Requete::Fermer { id: 4 } },
            Trame::Requete { sequence: 6, requete: Requete::Attacher { id: 4, taille } },
            Trame::Requete { sequence: 7, requete: Requete::Lister },
            Trame::Requete { sequence: 30, requete: Requete::Instantane { id: 4 } },
            Trame::Reponse { sequence: 31, reponse: Reponse::Octets(vec![27, b'c', 0, 200]) },
            Trame::Reponse { sequence: 32, reponse: Reponse::Octets(Vec::new()) },
            Trame::Requete {
                sequence: 8,
                requete: Requete::Chercher {
                    id: 4,
                    action: ActionRecherche::Precedente,
                    motif: "erreur : ça".into(),
                },
            },
            Trame::Requete {
                sequence: 9,
                requete: Requete::CopierSelection { id: 4, debut: position, fin: Position { ligne: 0, colonne: 79 } },
            },
            Trame::Requete { sequence: 10, requete: Requete::Redessiner { id: 4, avec_historique: true } },
            Trame::Requete { sequence: 11, requete: Requete::Arreter },
            Trame::Reponse { sequence: 1, reponse: Reponse::Fait },
            Trame::Reponse { sequence: 2, reponse: Reponse::Erreur("terminal inconnu".into()) },
            Trame::Reponse {
                sequence: 3,
                reponse: Reponse::Sessions(vec![
                    InfoSession { id: 1, vivant: true, llm: false, taille },
                    InfoSession { id: 2, vivant: false, llm: true, taille },
                ]),
            },
            Trame::Reponse { sequence: 4, reponse: Reponse::Sessions(Vec::new()) },
            Trame::Reponse { sequence: 5, reponse: Reponse::Texte("selection copiee".into()) },
            Trame::Reponse {
                sequence: 6,
                reponse: Reponse::Recherche { total: 12, index: Some(3), occurrence: Some(position) },
            },
            Trame::Reponse {
                sequence: 7,
                reponse: Reponse::Recherche { total: 0, index: None, occurrence: None },
            },
            Trame::Pousse(Pousse::Sortie { id: 4, octets: vec![27, 91, 50, 74] }),
            Trame::Pousse(Pousse::Redessin { id: 4, octets: vec![27, 99] }),
            Trame::Pousse(Pousse::PressePapier { id: 4, texte: "collé".into() }),
            Trame::Pousse(Pousse::Fini { id: 4 }),
            Trame::Pousse(Pousse::Panne { id: 4, message: "PTY perdu".into() }),
        ] {
            aller_retour(trame);
        }
    }

    /// Le cas du chemin le plus charge : une sortie binaire quelconque doit traverser sans
    /// encodage intermediaire. C'est ce qui justifie de ne pas passer par JSON.
    #[test]
    fn la_sortie_binaire_traverse_sans_alteration() {
        let octets: Vec<u8> = (0..=255u8).cycle().take(300_000).collect();
        aller_retour(Trame::Pousse(Pousse::Sortie { id: 1, octets }));
    }

    #[test]
    fn deux_trames_a_la_suite_se_lisent_l_une_apres_l_autre() {
        let mut flux = Trame::Requete { sequence: 1, requete: Requete::Lister }.encoder();
        flux.extend(Trame::Reponse { sequence: 1, reponse: Reponse::Fait }.encoder());
        let mut entree = flux.as_slice();
        assert!(matches!(Trame::lire(&mut entree).unwrap(), Some(Trame::Requete { .. })));
        assert!(matches!(Trame::lire(&mut entree).unwrap(), Some(Trame::Reponse { .. })));
        // Fin propre du flux : ni erreur, ni trame.
        assert_eq!(Trame::lire(&mut entree).unwrap(), None);
    }

    /// Une trame coupee en deux ne doit pas rendre un message a moitie rempli.
    #[test]
    fn une_trame_tronquee_est_une_erreur() {
        let entiere = Trame::Requete {
            sequence: 1,
            requete: Requete::Creer {
                id: 3,
                dossier: "/tmp".into(),
                taille: Taille { colonnes: 80, lignes: 24 },
                commande_initiale: None,
                ecran_initial: Vec::new(),
            },
        }
        .encoder();
        for coupe in [5, 9, entiere.len() - 1] {
            assert!(Trame::lire(&mut &entiere[..coupe]).is_err(), "coupe a {coupe}");
        }
    }

    #[test]
    fn un_genre_inconnu_est_refuse() {
        assert!(Trame::decoder(&[99]).is_err());
        assert!(Trame::decoder(&[1, 0, 0, 0, 1, 99]).is_err());
    }

    /// La poignee de main : dix octets de forme FIGEE, et le service parle en premier.
    #[test]
    fn le_preambule_se_relit() {
        let mut tuyau = Vec::new();
        ecrire_preambule(&mut tuyau).unwrap();
        assert_eq!(tuyau.len(), TAILLE_PREAMBULE);
        assert_eq!(lire_preambule(&mut tuyau.as_slice()), Ok(()));
    }

    #[test]
    fn un_interlocuteur_qui_n_est_pas_le_service_se_reconnait() {
        let bruit = b"HTTP/1.1 4";
        assert_eq!(lire_preambule(&mut &bruit[..]), Err(ErreurPoignee::PasNotreService));
    }

    /// Le chemin qui evite le « protocol version mismatch » de tmux : une erreur
    /// STRUCTUREE, avec les deux numeros, dans les deux sens.
    #[test]
    fn les_deux_sens_du_desaccord_de_version_sont_distingues() {
        assert_eq!(
            comparer_versions(1, 3),
            Err(ErreurPoignee::ServiceTropAncien { service: 1, application: 3 })
        );
        assert_eq!(
            comparer_versions(4, 3),
            Err(ErreurPoignee::ApplicationTropAncienne { service: 4, application: 3 })
        );
        assert_eq!(comparer_versions(3, 3), Ok(()));
    }

    /// Les messages doivent dire quoi faire, pas seulement que ca ne marche pas.
    #[test]
    fn le_message_de_desaccord_porte_les_deux_versions() {
        let texte = ErreurPoignee::ServiceTropAncien { service: 1, application: 2 }.to_string();
        assert!(texte.contains('1') && texte.contains('2'), "{texte}");
        assert!(texte.contains("ferme les terminaux"), "{texte}");
    }

    /// Un preambule coupe (service tue pendant la poignee de main) ne doit pas passer
    /// pour un accord.
    #[test]
    fn un_preambule_incomplet_est_une_erreur_de_tuyau() {
        let court = [b'C', b'K', b'P'];
        assert!(matches!(
            lire_preambule(&mut &court[..]),
            Err(ErreurPoignee::Tuyau(_))
        ));
    }
}
