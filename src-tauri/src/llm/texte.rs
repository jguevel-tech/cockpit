//! Les deux capacites qui passent par une API : repondre en texte, et transcrire de l'audio.
//!
//! ## OU PASSE LA COUTURE, ET POURQUOI LA
//!
//! Le fournisseur ne fait que l'appel reseau. **Tout ce qui est a nous reste a nous** : le
//! decoupage de l'audio en morceaux sous la limite de taille, la detection des pistes muettes,
//! le filtre des phrases que les modeles hallucinent sur du silence, la fusion des deux pistes
//! en dialogue. Ce sont des annees de petits pieges payes une fois ; les mettre derriere le
//! trait obligerait chaque nouveau fournisseur a les repayer, et le premier oubli sortirait un
//! compte rendu ou il manque la moitie de la reunion.
//!
//! ## Pourquoi des futurs en boite
//!
//! Ces appels sont asynchrones et le catalogue les expose par objet-trait. Un `async fn` dans
//! un trait ne s'utilise pas a travers `dyn` : on rend donc un futur en boite. C'est le prix
//! d'un catalogue ou un fournisseur se declare sans toucher au reste.

use std::future::Future;
use std::pin::Pin;

/// Un futur rendu par un objet-trait.
pub type Futur<'a, T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send + 'a>>;

/// Ce que le modele doit rendre : une reponse a une consigne et un contenu.
pub trait ModeleTexte: Send + Sync {
    /// Le modele employe faute de choix explicite. Un nom de modele appartient au
    /// fournisseur : `gpt-4o` ne veut rien dire ailleurs.
    fn modele_par_defaut(&self) -> &'static str;

    /// Une consigne, un contenu, une reponse.
    fn repondre<'a>(
        &'a self,
        client: &'a reqwest::Client,
        cle: &'a str,
        modele: &'a str,
        consigne: &'a str,
        contenu: &'a str,
    ) -> Futur<'a, String>;
}

/// Un bout de transcription, tel que l'API le rend.
pub struct SegmentTranscrit {
    /// Debut du segment, en secondes depuis le debut du morceau envoye.
    pub debut: f64,
    pub texte: String,
    /// Probabilite que ce segment ne soit pas de la parole. Les modeles hallucinent des
    /// phrases entieres sur du silence, et c'est ce nombre qui permet de les jeter.
    pub non_parole: f64,
}

pub trait Transcription: Send + Sync {
    /// La taille maximale d'un morceau audio accepte, en octets.
    ///
    /// C'est le fournisseur qui la connait, et c'est elle qui decide du decoupage : la fixer
    /// chez nous ferait echouer le premier fournisseur plus genereux ou plus strict, avec un
    /// « HTTP 413 » pour toute explication.
    fn taille_maximale(&self) -> usize;

    /// Transcrit UN morceau de WAV deja decoupe et deja juge non silencieux.
    fn transcrire<'a>(
        &'a self,
        client: &'a reqwest::Client,
        cle: &'a str,
        wav: Vec<u8>,
        langue: &'a str,
    ) -> Futur<'a, Vec<SegmentTranscrit>>;
}
