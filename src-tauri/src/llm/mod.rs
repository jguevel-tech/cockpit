//! Les fournisseurs d'IA que Cockpit sait utiliser.
//!
//! ## AJOUTER UN FOURNISSEUR = UNE DECLARATION DANS `CATALOGUE`
//!
//! ```ignore
//! static MISTRAL: Declaration = Declaration {
//!     id: "mistral", nom: "Mistral", commandes: &["mistral"], symbole: "◈", couleur: "#fa520f",
//! };
//! ```
//!
//! Rien d'autre a toucher : ni la detection d'un agent qui tourne, ni l'ecran des reglages,
//! ni les libelles. S'il sait faire davantage, on implemente le trait correspondant sur son
//! propre type — `Conversations`, `Abonnement`, `ModeleTexte`, `Transcription` — et le reste
//! de l'application le decouvre sans rien savoir de lui.
//!
//! ## Pourquoi des CAPACITES et non un enum de fournisseurs
//!
//! Cockpit etait ecrit autour d'un seul produit : le nom « claude » apparaissait dans un
//! module d'authentification, un lecteur de conversations, un bouton de terminal, un logo et
//! vingt-deux libelles. Ajouter un deuxieme fournisseur aurait demande de rouvrir chacun.
//!
//! Ici, chaque capacite est un trait et vaut `None` par defaut : **un fournisseur qui ne sait
//! pas faire quelque chose le DIT**, il ne le rate pas a l'execution. C'est ce que le frontend
//! interroge pour cacher un bouton au lieu de promettre ce qui n'existe pas — un bouton qui
//! promet ce que le fournisseur ne sait pas faire est un mensonge, meme involontaire.
//!
//! ## Le choix vit dans les REGLAGES, pas dans le code
//!
//! Un seul reglage (`llm_prefere`) et tout le reste le lit. Il reste LOCAL a la machine : les
//! reglages ne voyagent pas par la synchronisation (une cle d'API n'a rien a faire ailleurs),
//! et le logiciel doit marcher sans connexion.
//!
//! **Quand le fournisseur prefere ne sait pas faire ce qu'on demande, on ne le lui demande
//! pas** : `pour()` rend le premier du catalogue qui en est capable ET configure, et
//! l'interface AFFICHE lequel. Choisir Claude et voir les reunions partir chez OpenAI est
//! normal — Claude ne transcrit pas — mais ca ne doit pas se decouvrir apres coup.

pub mod abonnement;
pub mod conversations;
pub mod fournisseurs;
pub mod texte;

pub use abonnement::{Abonnement, EtatAbonnement};
pub use conversations::{Conversation, Conversations};
pub use texte::{ModeleTexte, Transcription};

use crate::storage::Database;
use serde::Serialize;

/// La cle du reglage qui porte le choix.
pub const CLE_PREFERE: &str = "llm_prefere";

/// Le fournisseur retenu quand rien n'a ete choisi.
///
/// Claude et non « le premier du catalogue » : les installations livrees fonctionnent avec lui,
/// et une mise a jour ne doit pas changer le comportement de quelqu'un qui n'a rien demande.
pub const DEFAUT: &str = "claude";

/// Ce qu'un fournisseur d'IA sait faire.
///
/// Tout est facultatif sauf son identite. Un fournisseur qui n'est qu'un CLI se declare avec
/// `Declaration` et n'implemente rien.
pub trait Fournisseur: Send + Sync {
    /// Identifiant stable, range en base. **Ne jamais le changer** : c'est lui qui est ecrit
    /// dans les reglages de quelqu'un, et le renommer perdrait son choix en silence.
    fn id(&self) -> &'static str;

    /// Le nom tel qu'on l'affiche. Pas dans les catalogues de traduction : « Claude » et
    /// « Gemini » s'ecrivent pareil dans toutes les langues.
    fn nom(&self) -> &'static str;

    /// Les noms sous lesquels son CLI tourne (basename, sans extension). Vide = pas de CLI.
    fn commandes(&self) -> &'static [&'static str] {
        &[]
    }

    /// Un caractere pour le designer a l'ecran. Un caractere et non une image : un logo par
    /// fournisseur voudrait dire un fichier a fournir, un droit d'usage a verifier et une
    /// taille a gerer pour chaque nouveau venu.
    fn symbole(&self) -> &'static str {
        "✳"
    }

    /// Sa couleur, pour le symbole. Ecrite ici et non dans un token de theme : elle appartient
    /// au fournisseur, pas a notre langage visuel.
    fn couleur(&self) -> &'static str {
        "#8a8f98"
    }

    /// Sait lister et reprendre les conversations passees d'un projet.
    fn conversations(&self) -> Option<&'static dyn Conversations> {
        None
    }

    /// Sait dire si l'on est connecte a son abonnement, et guider la connexion.
    fn abonnement(&self) -> Option<&'static dyn Abonnement> {
        None
    }

    /// Sait repondre a une question en texte (resume de reunion...).
    fn texte(&self) -> Option<&'static dyn ModeleTexte> {
        None
    }

    /// Sait transcrire de l'audio.
    fn transcription(&self) -> Option<&'static dyn Transcription> {
        None
    }

    /// Ses agents s'installent au format de plugins de Claude Code.
    ///
    /// **Cette capacite ne se generalise pas, et c'est assume** : l'onglet Plugins ecrit dans
    /// la configuration d'un AUTRE logiciel (`~/.claude/settings.json`, le format
    /// `.claude-plugin/`). Codex et Gemini n'ont pas ce concept ; inventer un equivalent pour
    /// eux serait inventer un format que personne ne lit. L'onglet dispara..t pour eux.
    fn plugins_claude_code(&self) -> bool {
        false
    }

    /// A besoin d'une cle d'API pour servir a quelque chose.
    fn cle_requise(&self) -> bool {
        false
    }
}

/// Un fournisseur qu'on se contente de DECLARER : un CLI, un symbole, une couleur.
///
/// C'est le cas le plus courant, et il ne doit rien couter : la detection d'un agent qui tourne
/// dans un terminal marche pour lui sans une ligne de plus.
pub struct Declaration {
    pub id: &'static str,
    pub nom: &'static str,
    pub commandes: &'static [&'static str],
    pub symbole: &'static str,
    pub couleur: &'static str,
}

impl Fournisseur for Declaration {
    fn id(&self) -> &'static str {
        self.id
    }
    fn nom(&self) -> &'static str {
        self.nom
    }
    fn commandes(&self) -> &'static [&'static str] {
        self.commandes
    }
    fn symbole(&self) -> &'static str {
        self.symbole
    }
    fn couleur(&self) -> &'static str {
        self.couleur
    }
}

/// LE CATALOGUE. Une ligne par fournisseur, et c'est le seul endroit a modifier.
///
/// L'ordre compte a un seul endroit : `pour()` prend le premier capable quand le fournisseur
/// prefere ne sait pas faire. Mettre en tete ceux qui savent le plus de choses.
pub static CATALOGUE: &[&dyn Fournisseur] = &[
    &fournisseurs::claude::CLAUDE,
    &fournisseurs::OPENAI,
    &fournisseurs::CODEX,
    &fournisseurs::GEMINI,
    &fournisseurs::AIDER,
    &fournisseurs::GOOSE,
    &fournisseurs::OPENCODE,
    &fournisseurs::COPILOT,
    &fournisseurs::CURSOR,
    &fournisseurs::AMP,
    &fournisseurs::QWEN,
    &fournisseurs::OLLAMA,
];

/// Le fournisseur portant cet identifiant.
pub fn par_id(id: &str) -> Option<&'static dyn Fournisseur> {
    CATALOGUE.iter().copied().find(|f| f.id() == id)
}

/// Le fournisseur choisi dans les reglages.
///
/// Un identifiant inconnu (reglage ecrit a la main, fournisseur retire du catalogue) retombe
/// sur le defaut au lieu de rendre `None` : aucune fonctionnalite ne doit s'eteindre parce
/// qu'une chaine de reglage ne correspond plus a rien.
pub fn prefere(db: &Database) -> &'static dyn Fournisseur {
    let choisi = db.get_setting(CLE_PREFERE).unwrap_or_default();
    if !choisi.is_empty() {
        if let Some(f) = par_id(&choisi) {
            return f;
        }
        log::warn!("fournisseur d'IA inconnu dans les reglages : {choisi}");
    }
    par_id(DEFAUT).or_else(|| CATALOGUE.first().copied()).expect("catalogue vide")
}

/// Retient le choix. Un identifiant inconnu est refuse : mieux vaut un message qu'un reglage
/// qui ne veut rien dire et une fonctionnalite qui s'eteint trois ecrans plus loin.
pub fn choisir(db: &Database, id: &str) -> Result<(), String> {
    if par_id(id).is_none() {
        return Err(format!("fournisseur d'IA inconnu : {id}"));
    }
    db.set_setting(CLE_PREFERE, id)
}

/// La cle d'API d'un fournisseur, quand elle est posee.
pub fn cle_api(db: &Database, id: &str) -> Option<String> {
    db.get_setting(&cle_du_reglage(id)).filter(|v| !v.trim().is_empty())
}

pub fn poser_cle_api(db: &Database, id: &str, valeur: &str) -> Result<(), String> {
    if par_id(id).is_none() {
        return Err(format!("fournisseur d'IA inconnu : {id}"));
    }
    db.set_setting(&cle_du_reglage(id), valeur.trim())
}

/// Le nom du reglage qui porte la cle d'un fournisseur.
///
/// `openai_api_key` pour OpenAI : c'est le nom historique, et il porte la cle de gens qui
/// enregistrent des reunions depuis des mois. Le renommer la perdrait sans un mot.
fn cle_du_reglage(id: &str) -> String {
    if id == "openai" {
        "openai_api_key".to_string()
    } else {
        format!("cle_api_{id}")
    }
}

/// Le fournisseur a utiliser pour une capacite donnee : le prefere s'il sait faire, sinon le
/// premier du catalogue qui sait faire ET qui est utilisable.
///
/// « Utilisable » veut dire : sa cle est posee quand il en demande une. Sans ce controle, on
/// choisirait un fournisseur pour echouer trois lignes plus loin sur une cle absente, en
/// laissant croire que c'est le prefere qui a echoue.
pub fn pour<T: ?Sized>(
    db: &Database,
    capacite: fn(&'static dyn Fournisseur) -> Option<&'static T>,
) -> Option<(&'static dyn Fournisseur, &'static T)> {
    let utilisable = |f: &'static dyn Fournisseur| !f.cle_requise() || cle_api(db, f.id()).is_some();

    let choisi = prefere(db);
    if let Some(capable) = capacite(choisi) {
        if utilisable(choisi) {
            return Some((choisi, capable));
        }
    }
    CATALOGUE
        .iter()
        .copied()
        .filter(|f| utilisable(*f))
        .find_map(|f| capacite(f).map(|c| (f, c)))
}

/// Tous les noms de CLI connus, toutes marques confondues.
///
/// C'est la SEULE liste : la detection d'un agent qui tourne dans un terminal la lit ici. Elle
/// vivait en dur a cote de la detection, donc declarer un fournisseur ne suffisait pas a le
/// faire reconnaitre.
pub fn commandes_connues() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = CATALOGUE.iter().flat_map(|f| f.commandes()).copied().collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Ce que le frontend a besoin de savoir d'un fournisseur pour n'afficher que ce qui existe.
#[derive(Serialize, Clone)]
pub struct Capacites {
    pub id: String,
    pub nom: String,
    pub symbole: String,
    pub couleur: String,
    /// Le CLI est-il installe sur cette machine ? Vide de sens pour un fournisseur d'API.
    pub cli: bool,
    /// A-t-il un CLI, tout simplement ?
    pub a_un_cli: bool,
    pub conversations: bool,
    pub abonnement: bool,
    pub texte: bool,
    pub transcription: bool,
    pub plugins: bool,
    pub cle_requise: bool,
    /// La cle est posee. **Jamais la cle elle-meme** : cette liste part au frontend a chaque
    /// ouverture des reglages, et une cle n'a pas a voyager pour repondre « oui ou non ».
    pub cle_posee: bool,
    pub prefere: bool,
}

pub fn catalogue_pour_le_frontend(db: &Database) -> Vec<Capacites> {
    let choisi = prefere(db).id();
    CATALOGUE
        .iter()
        .map(|f| Capacites {
            id: f.id().to_string(),
            nom: f.nom().to_string(),
            symbole: f.symbole().to_string(),
            couleur: f.couleur().to_string(),
            cli: f.commandes().iter().any(|c| dans_le_chemin(c)),
            a_un_cli: !f.commandes().is_empty(),
            conversations: f.conversations().is_some(),
            abonnement: f.abonnement().is_some(),
            texte: f.texte().is_some(),
            transcription: f.transcription().is_some(),
            plugins: f.plugins_claude_code(),
            cle_requise: f.cle_requise(),
            cle_posee: cle_api(db, f.id()).is_some(),
            prefere: f.id() == choisi,
        })
        .collect()
}

/// Ce programme est-il installe ? On PARCOURT `PATH`, on ne le lance pas.
///
/// Lancer `<cli> --version` pour chaque fournisseur ferait douze processus a chaque ouverture
/// des reglages — et sous Windows, douze consoles qui clignotent. La version, elle, ne se
/// demande qu'au fournisseur affiche, et c'est son implementation d'`Abonnement` qui la donne.
pub fn dans_le_chemin(programme: &str) -> bool {
    let Some(chemin) = std::env::var_os("PATH") else {
        return false;
    };
    // Sous Windows un programme s'appelle `claude.cmd` ou `claude.exe` : le nom nu ne
    // designe aucun fichier. La liste vient de PATHEXT, avec un repli sur les extensions
    // habituelles quand la variable manque.
    let suffixes: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string())
            .split(';')
            .filter(|e| !e.is_empty())
            .map(|e| e.to_lowercase())
            .chain(std::iter::once(String::new()))
            .collect()
    } else {
        vec![String::new()]
    };

    std::env::split_paths(&chemin).any(|dossier| {
        suffixes.iter().any(|suffixe| {
            let candidat = dossier.join(format!("{programme}{suffixe}"));
            candidat.is_file()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le catalogue est le seul endroit a modifier pour ajouter un fournisseur : il ne doit pas
    /// pouvoir contenir deux fois le meme identifiant, sinon `par_id` en choisit un au hasard
    /// et le reglage de quelqu'un designe l'autre.
    #[test]
    fn les_identifiants_sont_uniques_et_non_vides() {
        let mut vus = std::collections::HashSet::new();
        for f in CATALOGUE {
            assert!(!f.id().is_empty(), "un fournisseur sans identifiant");
            assert!(!f.nom().is_empty(), "{} n'a pas de nom", f.id());
            assert!(vus.insert(f.id()), "identifiant en double : {}", f.id());
            assert!(!f.symbole().is_empty(), "{} n'a pas de symbole", f.id());
        }
    }

    /// Un fournisseur sans CLI ni capacite ne servirait a rien : il n'apparaitrait dans aucun
    /// ecran et ne serait detecte dans aucun terminal. C'est le signe d'une declaration
    /// incomplete, pas d'un choix.
    #[test]
    fn chaque_fournisseur_apporte_quelque_chose() {
        for f in CATALOGUE {
            let capable = f.conversations().is_some()
                || f.abonnement().is_some()
                || f.texte().is_some()
                || f.transcription().is_some();
            assert!(
                !f.commandes().is_empty() || capable,
                "{} n'a ni CLI ni capacite : il ne sert a rien",
                f.id()
            );
        }
    }

    /// La detection d'un agent dans un terminal LIT le catalogue. Declarer un fournisseur doit
    /// suffire a le faire reconnaitre — c'est tout l'interet.
    #[test]
    fn les_commandes_viennent_du_catalogue() {
        let commandes = commandes_connues();
        for attendu in ["claude", "codex", "gemini", "ollama"] {
            assert!(commandes.contains(&attendu), "{attendu} devrait etre reconnu");
        }
        // Dedoublonnee et triee : deux fournisseurs peuvent partager un nom de CLI.
        let mut triee = commandes.clone();
        triee.sort_unstable();
        triee.dedup();
        assert_eq!(commandes, triee);
        // OpenAI n'a pas de CLI : rien a reconnaitre dans un terminal.
        assert!(!commandes.contains(&"openai"));
    }

    /// **L'ESSAI NE DOIT PAS ATTENDRE LE DEFAUT.** Un essai dont le resultat attendu est la
    /// valeur par defaut passe meme quand le mecanisme ne marche pas : on choisit donc un
    /// fournisseur qui n'est PAS celui par defaut.
    #[test]
    fn le_choix_est_retenu_et_relu() {
        let db = Database::new(":memory:").unwrap();
        assert_eq!(prefere(&db).id(), DEFAUT, "sans choix, le defaut");

        choisir(&db, "codex").unwrap();
        assert_eq!(prefere(&db).id(), "codex");
        assert_ne!("codex", DEFAUT, "l'essai ne prouverait rien avec le defaut");
    }

    #[test]
    fn un_choix_inconnu_est_refuse_et_ne_casse_rien() {
        let db = Database::new(":memory:").unwrap();
        assert!(choisir(&db, "hal9000").is_err());

        // Ecrit de force (reglage bricole a la main, ou fournisseur retire du catalogue) :
        // on retombe sur le defaut au lieu d'eteindre les fonctionnalites d'IA.
        db.set_setting(CLE_PREFERE, "hal9000").unwrap();
        assert_eq!(prefere(&db).id(), DEFAUT);
    }

    /// La regle qui evite de promettre ce qui n'existe pas : le prefere s'il sait faire, sinon
    /// le premier capable. Codex n'a pas d'API de transcription, OpenAI oui.
    #[test]
    fn une_capacite_absente_chez_le_prefere_va_au_premier_capable() {
        let db = Database::new(":memory:").unwrap();
        choisir(&db, "codex").unwrap();
        poser_cle_api(&db, "openai", "sk-essai").unwrap();

        let (fournisseur, _) = pour(&db, |f| f.transcription()).expect("un fournisseur capable");
        assert_eq!(fournisseur.id(), "openai");
    }

    /// Et sans cle, on ne le choisit pas : sinon on echoue plus loin sur une cle absente, en
    /// laissant croire que c'est le fournisseur prefere qui a echoue.
    #[test]
    fn un_fournisseur_sans_sa_cle_n_est_pas_choisi() {
        let db = Database::new(":memory:").unwrap();
        assert!(pour(&db, |f| f.transcription()).is_none());

        poser_cle_api(&db, "openai", "sk-essai").unwrap();
        assert!(pour(&db, |f| f.transcription()).is_some());
    }

    /// La cle d'OpenAI garde son nom de reglage historique : des gens en ont une depuis des
    /// mois, et la renommer la perdrait sans un mot.
    #[test]
    fn la_cle_openai_garde_son_nom_historique() {
        let db = Database::new(":memory:").unwrap();
        db.set_setting("openai_api_key", "sk-ancienne").unwrap();
        assert_eq!(cle_api(&db, "openai").as_deref(), Some("sk-ancienne"));
    }

    #[test]
    fn les_capacites_ne_transportent_jamais_la_cle() {
        let db = Database::new(":memory:").unwrap();
        poser_cle_api(&db, "openai", "sk-secrete").unwrap();

        let vue = catalogue_pour_le_frontend(&db);
        let openai = vue.iter().find(|c| c.id == "openai").unwrap();
        assert!(openai.cle_posee);
        let json = serde_json::to_string(&vue).unwrap();
        assert!(!json.contains("sk-secrete"), "une cle ne doit pas partir au frontend");
    }

    /// Un programme qui n'existe pas ne doit pas etre annonce comme installe. On ne peut pas
    /// affirmer l'inverse dans un essai : la machine qui l'execute n'a peut-etre aucun CLI.
    #[test]
    fn un_programme_absent_n_est_pas_dans_le_chemin() {
        assert!(!dans_le_chemin("cockpit-programme-qui-n-existe-pas"));
    }
}
