//! Ce qu'il reste avant la limite du fournisseur, et quand le compteur repart.
//!
//! **POURQUOI CETTE CAPACITE EXISTE.** L'abonnement dit « connecte, formule Max » ; il ne dit
//! pas qu'il reste treize minutes avant de se faire couper au milieu d'une session. Cette
//! capacite-la repond a la seule question qu'on se pose vraiment en travaillant : est-ce que je
//! peux lancer ce gros travail maintenant, ou est-ce que j'attends la remise a zero.
//!
//! **ON NE DETIENT AUCUN JETON.** Le fournisseur garde les siens dans son fichier a lui ; on le
//! LIT au moment de demander, et rien n'est recopie en base. Meme regle que l'abonnement.
//!
//! **UN FOURNISSEUR QUI NE SAIT PAS REPOND `None`, ET L'INTERFACE N'AFFICHE RIEN.** Pas de
//! pastille grise, pas de « indisponible » : un indicateur qui ne mesure rien est pire que pas
//! d'indicateur.

use serde::Serialize;

/// Une fenetre de limitation : la part consommee, et l'instant ou elle repart a zero.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Fenetre {
    /// Ce que la fenetre couvre, en clair et non traduit : le frontend choisit son libelle.
    /// `session` pour la fenetre courte (cinq heures chez Claude), `semaine` pour la longue.
    pub cle: &'static str,
    /// De 0 a 100. Au-dela de 100 le fournisseur coupe.
    pub pourcentage: f64,
    /// Epoch SECONDES. `None` quand le fournisseur ne le dit pas : on affiche alors la part
    /// consommee sans promettre une heure qu'on ne connait pas.
    pub remise_a_zero: Option<i64>,
}

/// L'etat complet, tel qu'il part au frontend.
#[derive(Serialize, Clone)]
pub struct EtatConsommation {
    pub fournisseur: String,
    pub nom: String,
    /// Le fournisseur sait-il seulement parler de consommation ?
    pub gere: bool,
    pub fenetres: Vec<Fenetre>,
    /// Pourquoi on n'a pas su regarder. **« Rien a montrer » et « la mesure a echoue » sont
    /// deux choses differentes** : sans ce champ, une panne reseau se lirait « tout va bien ».
    pub probleme: Option<String>,
}

/// Ce qu'un fournisseur sait dire de sa propre consommation.
///
/// Le futur est en boite pour la meme raison que dans `texte.rs` : un `async fn` dans un trait
/// ne s'utilise pas a travers `dyn`, et le catalogue expose ses capacites par objet-trait.
pub trait Consommation: Send + Sync {
    /// Les fenetres, ou la raison pour laquelle on n'a rien.
    fn fenetres<'a>(&'a self, client: &'a reqwest::Client) -> super::texte::Futur<'a, Vec<Fenetre>>;
}

/// Lit un pourcentage dans la reponse d'un fournisseur.
///
/// Deux noms coexistent chez Claude (`utilization` et `used_percentage`) et l'un ou l'autre
/// manque selon la formule : n'en lire qu'un rendait « 0 % » a des comptes largement entames.
pub fn pourcentage_de(valeur: &serde_json::Value) -> Option<f64> {
    for nom in ["utilization", "used_percentage", "percent"] {
        if let Some(n) = valeur.get(nom).and_then(|v| v.as_f64()) {
            if n.is_finite() {
                return Some(n.clamp(0.0, 100.0));
            }
        }
    }
    None
}

/// Lit l'instant de remise a zero, en epoch SECONDES.
///
/// **TROIS FORMES POUR LA MEME DONNEE**, et les trois arrivent : un nombre de secondes, un
/// nombre de millisecondes, ou une date ISO. Un seuil separe les deux premieres : 10 000 000 000
/// est apres l'an 2286 en secondes, et avant 1971 en millisecondes — aucune valeur reelle ne
/// tombe des deux cotes.
pub fn remise_a_zero_de(valeur: &serde_json::Value) -> Option<i64> {
    if let Some(n) = valeur.get("resets_at").and_then(|v| v.as_i64()) {
        return Some(if n > 10_000_000_000 { n / 1000 } else { n });
    }
    let texte = valeur.get("resets_at")?.as_str()?;
    if let Ok(n) = texte.trim().parse::<i64>() {
        return Some(if n > 10_000_000_000 { n / 1000 } else { n });
    }
    // Une date ISO 8601, telle que l'API la rend quand elle ne rend pas un nombre.
    chrono_minimal(texte)
}

/// Convertit une date ISO 8601 en epoch secondes, sans dependance supplementaire.
///
/// Le projet n'embarque pas de bibliotheque de dates cote backend, et en ajouter une pour une
/// seule chaine serait disproportionne. La forme rendue par l'API est figee :
/// `2026-09-11T14:30:00Z`, eventuellement avec des fractions de seconde.
fn chrono_minimal(texte: &str) -> Option<i64> {
    let texte = texte.trim();
    let (date, reste) = texte.split_once('T')?;
    let mut parties_date = date.split('-');
    let annee: i64 = parties_date.next()?.parse().ok()?;
    let mois: i64 = parties_date.next()?.parse().ok()?;
    let jour: i64 = parties_date.next()?.parse().ok()?;
    let heure_brute = reste.trim_end_matches('Z');
    let heure_brute = heure_brute.split('.').next()?;
    let mut parties_heure = heure_brute.split(':');
    let heures: i64 = parties_heure.next()?.parse().ok()?;
    let minutes: i64 = parties_heure.next()?.parse().ok()?;
    let secondes: i64 = parties_heure.next().unwrap_or("0").parse().ok()?;

    // Jours depuis l'epoch, par le calendrier civil (algorithme de Howard Hinnant).
    let annee_decalee = if mois <= 2 { annee - 1 } else { annee };
    let ere = if annee_decalee >= 0 { annee_decalee } else { annee_decalee - 399 } / 400;
    let annee_dans_ere = annee_decalee - ere * 400;
    let jour_dans_annee = (153 * (mois + if mois > 2 { -3 } else { 9 }) + 2) / 5 + jour - 1;
    let jour_dans_ere =
        annee_dans_ere * 365 + annee_dans_ere / 4 - annee_dans_ere / 100 + jour_dans_annee;
    let jours = ere * 146_097 + jour_dans_ere - 719_468;

    Some(jours * 86_400 + heures * 3_600 + minutes * 60 + secondes)
}

/// Construit une fenetre a partir d'un bloc de la reponse. `None` si le bloc ne porte pas de
/// pourcentage : une fenetre sans chiffre n'a rien a afficher.
pub fn fenetre_depuis(
    cle: &'static str,
    bloc: Option<&serde_json::Value>,
) -> Option<Fenetre> {
    let bloc = bloc?;
    Some(Fenetre {
        cle,
        pourcentage: pourcentage_de(bloc)?,
        remise_a_zero: remise_a_zero_de(bloc),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn les_deux_noms_de_pourcentage_sont_lus() {
        assert_eq!(pourcentage_de(&json!({ "utilization": 36 })), Some(36.0));
        assert_eq!(pourcentage_de(&json!({ "used_percentage": 23.5 })), Some(23.5));
        assert_eq!(pourcentage_de(&json!({ "percent": 12 })), Some(12.0));
        assert_eq!(pourcentage_de(&json!({ "autre": 12 })), None);
    }

    #[test]
    fn un_pourcentage_aberrant_est_borne() {
        // Le fournisseur a deja rendu des valeurs au-dela de 100 ; une jauge a 340 % ne veut
        // rien dire a l'ecran.
        assert_eq!(pourcentage_de(&json!({ "utilization": 340 })), Some(100.0));
        assert_eq!(pourcentage_de(&json!({ "utilization": -3 })), Some(0.0));
    }

    /// L'instant de reference : `2026-02-02T04:00:00Z`. Verifie hors de Rust plutot que
    /// recalcule ici — un essai qui refait le calcul qu'il verifie ne verifie rien.
    const REFERENCE: i64 = 1_770_004_800;

    #[test]
    fn les_trois_formes_de_date_donnent_le_meme_instant() {
        assert_eq!(remise_a_zero_de(&json!({ "resets_at": REFERENCE })), Some(REFERENCE));
        assert_eq!(
            remise_a_zero_de(&json!({ "resets_at": REFERENCE * 1000 })),
            Some(REFERENCE),
            "des millisecondes doivent etre reconnues comme telles"
        );
        assert_eq!(
            remise_a_zero_de(&json!({ "resets_at": "2026-02-02T04:00:00Z" })),
            Some(REFERENCE)
        );
    }

    #[test]
    fn une_date_iso_avec_fractions_passe_aussi() {
        assert_eq!(
            remise_a_zero_de(&json!({ "resets_at": "2026-02-02T04:00:00.512Z" })),
            Some(REFERENCE)
        );
    }

    /// Une date hors de l'annee en cours, pour que l'algorithme du calendrier soit vraiment
    /// eprouve : un 29 fevrier, et une annee seculaire non bissextile.
    #[test]
    fn le_calendrier_tient_sur_les_cas_penibles() {
        // 2024-02-29T12:00:00Z et 1900-03-01T00:00:00Z, verifies hors de Rust.
        assert_eq!(
            remise_a_zero_de(&json!({ "resets_at": "2024-02-29T12:00:00Z" })),
            Some(1_709_208_000)
        );
        assert_eq!(
            remise_a_zero_de(&json!({ "resets_at": "1900-03-01T00:00:00Z" })),
            Some(-2_203_891_200)
        );
    }

    #[test]
    fn une_fenetre_sans_chiffre_n_existe_pas() {
        let bloc = json!({ "resets_at": 1_770_000_000_i64 });
        assert_eq!(fenetre_depuis("session", Some(&bloc)), None);
        assert_eq!(fenetre_depuis("session", None), None);
    }

    #[test]
    fn une_fenetre_complete_porte_sa_cle() {
        let bloc = json!({ "utilization": 73, "resets_at": 1_770_000_000_i64 });
        assert_eq!(
            fenetre_depuis("semaine", Some(&bloc)),
            Some(Fenetre {
                cle: "semaine",
                pourcentage: 73.0,
                remise_a_zero: Some(1_770_000_000)
            })
        );
    }
}
