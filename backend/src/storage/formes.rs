//! La forme de ce qui voyage : quelles colonnes partent, et lesquelles restent ici.
//!
//! Une description par table plutot que huit fonctions ecrites a la main : les huit auraient
//! diverge, et une colonne oubliee dans l'une d'elles ne se voit pas — la donnee arrive
//! simplement vide sur l'autre machine, sans erreur.
//!
//! **Ce qui ne part PAS est aussi important que ce qui part.** Le chemin d'un projet n'existe
//! que sur cette machine ; l'envoyer ferait pointer l'autre poste vers un dossier absent, et
//! l'onglet Fichiers montrerait une erreur au lieu d'un projet a rattacher.

use super::db::Database;
use rusqlite::Result;

/// Une reference d'une table vers une autre.
///
/// Elle voyage en identifiant GLOBAL : un identifiant local ne veut rien dire ailleurs.
pub struct Reference {
    /// Colonne locale, entiere.
    pub colonne: &'static str,
    /// Type vise, pour retrouver l'identifiant global.
    pub cible: &'static str,
    /// Nom sous lequel la reference voyage.
    pub cle: &'static str,
}

pub struct Forme {
    pub type_: &'static str,
    pub table: &'static str,
    /// Colonnes qui voyagent telles quelles, sous leur propre nom.
    pub colonnes: &'static [&'static str],
    pub references: &'static [Reference],
    /// Colonnes obligatoires qui ne voyagent PAS, et la valeur a poser en creant la ligne
    /// ici. Sans ca l'insertion echouerait sur une contrainte, et la donnee n'arriverait
    /// jamais — sans que personne ne comprenne pourquoi.
    pub defauts: &'static [(&'static str, &'static str)],
}

/// L'ordre compte : un parent doit pouvoir exister avant son enfant. Ce qui reste non resolu
/// apres un passage est recousu ensuite.
pub const FORMES: &[Forme] = &[
    Forme {
        type_: "dossier_projet",
        table: "project_folders",
        colonnes: &["name", "position"],
        references: &[Reference { colonne: "parent_id", cible: "dossier_projet", cle: "parent" }],
        defauts: &[],
    },
    Forme {
        type_: "projet",
        table: "projects",
        // `path` est absent EXPRES : le dossier d'un projet n'existe que sur cette machine.
        colonnes: &["name", "compose_file", "description", "depends_on", "position", "summary_prompt"],
        references: &[Reference { colonne: "folder_id", cible: "dossier_projet", cle: "dossier" }],
        // Un projet venu d'une autre machine arrive SANS dossier : il faudra le rattacher
        // ici. Mieux vaut un projet a rattacher qu'un chemin qui n'existe pas.
        defauts: &[("path", "")],
    },
    Forme {
        type_: "tache",
        table: "todos",
        colonnes: &["project", "text", "done", "position", "due_date", "progress"],
        references: &[],
        defauts: &[],
    },
    Forme {
        type_: "lien",
        table: "urls",
        colonnes: &["project", "label", "url", "position"],
        references: &[],
        defauts: &[],
    },
    Forme {
        type_: "commande",
        table: "project_commands",
        colonnes: &["project", "label", "command", "position"],
        references: &[],
        defauts: &[],
    },
    Forme {
        type_: "dossier_note",
        table: "note_folders",
        colonnes: &["project", "name", "position"],
        references: &[Reference { colonne: "parent_id", cible: "dossier_note", cle: "parent" }],
        defauts: &[],
    },
    Forme {
        type_: "fichier_note",
        table: "note_files",
        colonnes: &["project", "name", "content", "position"],
        references: &[Reference { colonne: "folder_id", cible: "dossier_note", cle: "dossier" }],
        defauts: &[],
    },
    Forme {
        type_: "note",
        table: "notes",
        colonnes: &["project", "content"],
        references: &[],
        defauts: &[],
    },
];

pub fn forme_de(type_: &str) -> Option<&'static Forme> {
    FORMES.iter().find(|f| f.type_ == type_)
}

impl Database {
    /// Le contenu d'une ligne, tel qu'il partira.
    ///
    /// Rend `None` si la ligne n'existe plus : entre le moment ou le journal l'a notee et
    /// celui de l'envoi, elle a pu etre supprimee. Ce n'est pas une panne.
    pub fn contenu_a_envoyer(&self, type_: &str, uuid: &str) -> Result<Option<String>> {
        let Some(forme) = forme_de(type_) else { return Ok(None) };
        let Some(local) = self.local_de(type_, uuid) else { return Ok(None) };

        let mut morceaux: Vec<String> = forme
            .colonnes
            .iter()
            .map(|c| format!("'{c}', t.{c}"))
            .collect();
        for r in forme.references {
            morceaux.push(format!(
                "'{}', (SELECT i.uuid FROM sync_identite i WHERE i.type = '{}' AND i.local_id = t.{})",
                r.cle, r.cible, r.colonne
            ));
        }

        let sql = format!(
            "SELECT json_object({}) FROM {} t WHERE t.id = ?1",
            morceaux.join(", "),
            forme.table
        );

        self.conn()
            .query_row(&sql, [local], |l| l.get::<_, String>(0))
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                autre => Err(autre),
            })
    }


    /// Ecrit ici une modification venue d'ailleurs.
    ///
    /// A appeler journalisation SUSPENDUE : sans ca, la ligne qu'on vient d'ecrire serait
    /// notee comme modifiee et renvoyee aussitot au serveur, indefiniment.
    ///
    /// Une reference dont la cible n'est pas encore arrivee est posee a NULL et non refusee :
    /// refuser perdrait la donnee, alors que la cible arrive peut-etre dans le meme lot. La
    /// couture d'apres remet les references en place.
    pub fn appliquer_un_changement(
        &self,
        type_: &str,
        uuid: &str,
        contenu: &serde_json::Value,
    ) -> Result<()> {
        let Some(forme) = forme_de(type_) else { return Ok(()) };

        // TOUTES les recherches se font AVANT de prendre la connexion. `conn()` rend un verrou
        // non reentrant : le prendre puis appeler une fonction qui le reprend bloque le
        // programme pour de bon, sans message. Les essais unitaires ne le voyaient pas — ils
        // ne le prenaient qu'une fois.
        //
        // **Seules les colonnes PRESENTES dans le message sont ecrites.** Une colonne absente
        // n'est pas une colonne vide : a la creation elle prend la valeur par defaut de la
        // table — sans quoi toute colonne obligatoire ferait echouer l'insertion — et a la
        // modification elle reste telle quelle. C'est aussi ce qui permet a une machine plus
        // ancienne, qui ignore une colonne recente, de ne pas l'effacer en passant.
        let mut noms: Vec<&str> = Vec::new();
        let mut valeurs: Vec<rusqlite::types::Value> = Vec::new();

        for c in forme.colonnes {
            if let Some(v) = contenu.get(*c) {
                noms.push(c);
                valeurs.push(en_valeur(Some(v)));
            }
        }
        for r in forme.references {
            if let Some(v) = contenu.get(r.cle) {
                noms.push(r.colonne);
                let local = v.as_str().and_then(|u| self.local_de(r.cible, u));
                valeurs.push(match local {
                    Some(id) => rusqlite::types::Value::Integer(id),
                    None => rusqlite::types::Value::Null,
                });
            }
        }

        let existant = self.local_de(type_, uuid);

        let conn = self.conn();
        let params: Vec<&dyn rusqlite::ToSql> =
            valeurs.iter().map(|v| v as &dyn rusqlite::ToSql).collect();

        if let Some(local) = existant {
            if noms.is_empty() {
                return Ok(());
            }
            let affectations: Vec<String> = noms
                .iter()
                .enumerate()
                .map(|(i, n)| format!("{n} = ?{}", i + 1))
                .collect();
            let sql = format!(
                "UPDATE {} SET {} WHERE id = ?{}",
                forme.table,
                affectations.join(", "),
                noms.len() + 1
            );
            let mut tous = params;
            tous.push(&local);
            conn.execute(&sql, tous.as_slice())?;
            return Ok(());
        }

        let mut noms_insertion: Vec<String> = noms.iter().map(|n| (*n).to_string()).collect();
        let mut reperes: Vec<String> = (1..=noms.len()).map(|i| format!("?{i}")).collect();
        for (colonne, valeur) in forme.defauts {
            noms_insertion.push((*colonne).to_string());
            reperes.push(format!("'{}'", valeur.replace('\'', "''")));
        }

        // Un message qui ne porte AUCUNE colonne connue : il n'y a rien a creer, et l'insertion
        // sans colonne n'est pas du SQL valide. On le dit plutot que de fabriquer une ligne
        // vide que personne ne saurait expliquer.
        if noms_insertion.is_empty() {
            log::warn!("synchro : « {type_} » {uuid} ne porte aucune colonne connue, ignore");
            return Ok(());
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            forme.table,
            noms_insertion.join(", "),
            reperes.join(", ")
        );
        conn.execute(&sql, params.as_slice())?;
        let local = conn.last_insert_rowid();
        drop(conn);

        // Le declencheur a pose un identifiant tire au hasard ; on lui substitue CELUI du
        // serveur, sinon la meme donnee existerait sous deux identites et reviendrait en
        // double a chaque synchronisation.
        self.associer_uuid(type_, local, uuid)?;
        Ok(())
    }

    /// Supprime ici ce qui a ete supprime ailleurs.
    pub fn appliquer_une_suppression(&self, type_: &str, uuid: &str) -> Result<()> {
        let Some(forme) = forme_de(type_) else { return Ok(()) };
        let Some(local) = self.local_de(type_, uuid) else { return Ok(()) };

        self.conn()
            .execute(&format!("DELETE FROM {} WHERE id = ?1", forme.table), [local])?;
        Ok(())
    }

    /// Remet en place les references dont la cible est arrivee apres elles.
    ///
    /// Sans cette passe, un dossier recu avant son parent resterait a la racine pour toujours :
    /// rien ne le remettrait au bon endroit, et l'utilisateur verrait son rangement a plat.
    pub fn recoudre_les_references(&self, recus: &[(String, String, serde_json::Value)]) -> Result<()> {
        // Les recherches d'abord, les ecritures ensuite : `conn()` rend un verrou non
        // reentrant, et chercher pendant qu'on le tient bloquerait le programme.
        let mut a_ecrire: Vec<(String, String, i64, i64)> = Vec::new();

        for (type_, uuid, contenu) in recus {
            let Some(forme) = forme_de(type_) else { continue };
            if forme.references.is_empty() {
                continue;
            }
            let Some(local) = self.local_de(type_, uuid) else { continue };

            for r in forme.references {
                if let Some(id) = contenu
                    .get(r.cle)
                    .and_then(|v| v.as_str())
                    .and_then(|u| self.local_de(r.cible, u))
                {
                    a_ecrire.push((forme.table.to_string(), r.colonne.to_string(), id, local));
                }
            }
        }

        let conn = self.conn();
        for (table, colonne, cible, local) in a_ecrire {
            conn.execute(
                &format!("UPDATE {table} SET {colonne} = ?1 WHERE id = ?2 AND {colonne} IS NULL"),
                rusqlite::params![cible, local],
            )?;
        }
        Ok(())
    }
}

/// Convertit une valeur JSON en valeur SQLite.
///
/// Un booleen devient 0 ou 1 : SQLite n'a pas de type booleen, et les colonnes concernees
/// (`done`) sont des entiers. Sans cette conversion, une tache cochee arriverait decochee.
fn en_valeur(v: Option<&serde_json::Value>) -> rusqlite::types::Value {
    use rusqlite::types::Value;
    match v {
        None | Some(serde_json::Value::Null) => Value::Null,
        Some(serde_json::Value::Bool(b)) => Value::Integer(i64::from(*b)),
        Some(serde_json::Value::Number(n)) => n
            .as_i64()
            .map(Value::Integer)
            .or_else(|| n.as_f64().map(Value::Real))
            .unwrap_or(Value::Null),
        Some(serde_json::Value::String(s)) => Value::Text(s.clone()),
        Some(autre) => Value::Text(autre.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_chemin_d_un_projet_ne_voyage_pas() {
        let projet = forme_de("projet").expect("la forme du projet existe");
        assert!(
            !projet.colonnes.contains(&"path"),
            "envoyer le chemin ferait pointer l'autre machine vers un dossier absent",
        );
    }

    #[test]
    fn chaque_forme_correspond_a_un_type_synchronise() {
        for forme in FORMES {
            assert!(
                super::super::synchro::TYPES.iter().any(|(t, table)| *t == forme.type_ && *table == forme.table),
                "la forme « {} » ne correspond a aucun type synchronise : elle ne partirait jamais",
                forme.type_,
            );
        }
        assert_eq!(
            FORMES.len(),
            super::super::synchro::TYPES.len(),
            "un type sans forme est journalise mais jamais envoye — une donnee perdue en silence",
        );
    }

    #[test]
    fn le_contenu_porte_les_colonnes_et_les_references_en_identifiant_global() {
        let db = Database::new(":memory:").unwrap();
        let conn = db.conn();
        conn.execute("INSERT INTO project_folders (name, position) VALUES ('Clients', 0)", [])
            .unwrap();
        let dossier = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO projects (name, path, folder_id) VALUES ('facturation', '/home/moi/fact', ?1)",
            [dossier],
        )
        .unwrap();
        let projet = conn.last_insert_rowid();
        drop(conn);

        let uuid_projet = db.uuid_de("projet", projet).unwrap();
        let uuid_dossier = db.uuid_de("dossier_projet", dossier).unwrap();
        let contenu = db.contenu_a_envoyer("projet", &uuid_projet).unwrap().unwrap();

        assert!(contenu.contains("\"name\":\"facturation\""));
        assert!(!contenu.contains("/home/moi/fact"), "le chemin ne doit pas partir");
        assert!(
            contenu.contains(&format!("\"dossier\":\"{uuid_dossier}\"")),
            "la reference doit voyager en identifiant global, pas en numero local : {contenu}",
        );
    }

    #[test]
    fn une_ligne_disparue_entre_temps_ne_fait_pas_echouer_l_envoi() {
        let db = Database::new(":memory:").unwrap();
        assert!(db.contenu_a_envoyer("projet", "inconnu").unwrap().is_none());
    }

    fn recue(json: &str) -> serde_json::Value {
        serde_json::from_str(json).unwrap()
    }

    /// Une base qui joue le role de la MACHINE D'ARRIVEE : journalisation suspendue, comme
    /// pendant une vraie application.
    fn arrivee() -> Database {
        let db = Database::new(":memory:").unwrap();
        db.suspendre_la_journalisation(true).unwrap();
        db
    }

    #[test]
    fn une_donnee_recue_est_creee_sous_l_identifiant_du_serveur() {
        let db = arrivee();
        let uuid = "11111111-1111-4111-8111-111111111111";

        db.appliquer_un_changement("projet", uuid, &recue(r#"{"name":"venu-d-ailleurs","position":3}"#))
            .unwrap();

        let local = db.local_de("projet", uuid).expect("la ligne existe et porte l'identifiant du serveur");
        let (nom, chemin): (String, String) = db
            .conn()
            .query_row("SELECT name, path FROM projects WHERE id = ?1", [local], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!(nom, "venu-d-ailleurs");
        assert_eq!(chemin, "", "le dossier est a rattacher sur cette machine");
        assert!(
            db.changements_en_attente().unwrap().is_empty(),
            "journaliser ce qu'on vient de recevoir le renverrait aussitot",
        );
    }

    /// Le declencheur pose un identifiant tire au hasard a l'insertion. S'il restait, la meme
    /// donnee existerait sous deux identites et reviendrait en double a chaque passage.
    #[test]
    fn l_identifiant_tire_par_le_declencheur_est_remplace_par_celui_du_serveur() {
        let db = arrivee();
        let uuid = "22222222-2222-4222-8222-222222222222";

        db.appliquer_un_changement("tache", uuid, &recue(r#"{"project":"p","text":"faire","done":false}"#))
            .unwrap();
        let local = db.local_de("tache", uuid).unwrap();

        assert_eq!(db.uuid_de("tache", local).as_deref(), Some(uuid));
        let combien: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM sync_identite WHERE type = 'tache'", [], |l| l.get(0))
            .unwrap();
        assert_eq!(combien, 1, "deux identites pour une seule ligne feraient un doublon a chaque synchro");
    }

    #[test]
    fn recevoir_deux_fois_la_meme_chose_ne_cree_pas_de_doublon() {
        let db = arrivee();
        let uuid = "33333333-3333-4333-8333-333333333333";

        db.appliquer_un_changement("lien", uuid, &recue(r#"{"project":"p","label":"Site","url":"https://a.test"}"#))
            .unwrap();
        db.appliquer_un_changement("lien", uuid, &recue(r#"{"project":"p","label":"Site v2","url":"https://b.test"}"#))
            .unwrap();

        let (combien, libelle): (i64, String) = db
            .conn()
            .query_row("SELECT COUNT(*), MAX(label) FROM urls", [], |l| Ok((l.get(0)?, l.get(1)?)))
            .unwrap();
        assert_eq!(combien, 1);
        assert_eq!(libelle, "Site v2", "la seconde reception doit modifier, pas ajouter");
    }

    /// SQLite n'a pas de type booleen : sans conversion, une tache cochee arriverait decochee.
    #[test]
    fn une_tache_cochee_arrive_cochee() {
        let db = arrivee();
        let uuid = "44444444-4444-4444-8444-444444444444";

        db.appliquer_un_changement("tache", uuid, &recue(r#"{"project":"p","text":"finie","done":true}"#))
            .unwrap();

        // L'identifiant est lu AVANT : `db.conn()` est evalue avant les arguments, donc une
        // recherche placee dans les arguments s'executerait verrou tenu — et bloquerait.
        let local = db.local_de("tache", uuid).unwrap();
        let fait: i64 = db
            .conn()
            .query_row("SELECT done FROM todos WHERE id = ?1", [local], |l| l.get(0))
            .unwrap();
        assert_eq!(fait, 1);
    }

    /// Un enfant recu avant son parent ne doit pas etre perdu : il arrive a la racine, et la
    /// couture le remet en place quand le parent est la.
    #[test]
    fn un_enfant_recu_avant_son_parent_est_recousu_ensuite() {
        let db = arrivee();
        let parent = "55555555-5555-4555-8555-555555555555";
        let enfant = "66666666-6666-4666-8666-666666666666";
        let contenu_enfant = recue(&format!(r#"{{"name":"Interne","position":0,"parent":"{parent}"}}"#));

        db.appliquer_un_changement("dossier_projet", enfant, &contenu_enfant).unwrap();
        let id_enfant = db.local_de("dossier_projet", enfant).unwrap();
        let avant: Option<i64> = db
            .conn()
            .query_row("SELECT parent_id FROM project_folders WHERE id = ?1", [id_enfant], |l| l.get(0))
            .unwrap();
        assert!(avant.is_none(), "sans son parent, l'enfant arrive a la racine plutot que d'etre refuse");

        db.appliquer_un_changement("dossier_projet", parent, &recue(r#"{"name":"Clients","position":0}"#))
            .unwrap();
        db.recoudre_les_references(&[("dossier_projet".into(), enfant.into(), contenu_enfant)])
            .unwrap();

        let apres: Option<i64> = db
            .conn()
            .query_row("SELECT parent_id FROM project_folders WHERE id = ?1", [id_enfant], |l| l.get(0))
            .unwrap();
        assert_eq!(
            apres,
            db.local_de("dossier_projet", parent),
            "sans couture, le rangement de l'utilisateur resterait a plat pour toujours",
        );
    }

    #[test]
    fn une_suppression_recue_retire_la_ligne() {
        let db = arrivee();
        let uuid = "77777777-7777-4777-8777-777777777777";
        db.appliquer_un_changement("tache", uuid, &recue(r#"{"project":"p","text":"a-effacer"}"#))
            .unwrap();

        db.appliquer_une_suppression("tache", uuid).unwrap();

        let reste: i64 = db.conn().query_row("SELECT COUNT(*) FROM todos", [], |l| l.get(0)).unwrap();
        assert_eq!(reste, 0);
    }

    /// Une colonne absente du message n'est pas une colonne vide : a la creation elle prend la
    /// valeur par defaut de la table, sinon toute colonne obligatoire ferait echouer
    /// l'insertion et la donnee n'arriverait jamais.
    #[test]
    fn une_colonne_absente_prend_la_valeur_par_defaut() {
        let db = arrivee();
        let uuid = "88888888-8888-4888-8888-888888888888";

        db.appliquer_un_changement("tache", uuid, &recue(r#"{"project":"p","text":"minimale"}"#))
            .unwrap();

        let local = db.local_de("tache", uuid).unwrap();
        let (position, faite): (i64, i64) = db
            .conn()
            .query_row("SELECT position, done FROM todos WHERE id = ?1", [local], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!((position, faite), (0, 0));
    }

    /// Une machine plus ancienne, qui ignore une colonne recente, ne doit pas l'effacer en
    /// passant : ce qu'elle ne mentionne pas reste tel quel.
    #[test]
    fn une_colonne_absente_d_une_modification_reste_intacte() {
        let db = arrivee();
        let uuid = "99999999-9999-4999-8999-999999999999";
        db.appliquer_un_changement("tache", uuid, &recue(r#"{"project":"p","text":"avec","progress":70}"#))
            .unwrap();

        db.appliquer_un_changement("tache", uuid, &recue(r#"{"project":"p","text":"sans"}"#))
            .unwrap();

        let local = db.local_de("tache", uuid).unwrap();
        let (texte, avancement): (String, i64) = db
            .conn()
            .query_row("SELECT text, progress FROM todos WHERE id = ?1", [local], |l| {
                Ok((l.get(0)?, l.get(1)?))
            })
            .unwrap();
        assert_eq!(texte, "sans");
        assert_eq!(avancement, 70, "ce qui n'est pas mentionne ne doit pas etre efface");
    }

    #[test]
    fn supprimer_ce_qui_n_existe_pas_ici_ne_fait_rien() {
        let db = arrivee();
        // Le cas arrive normalement : une donnee creee puis supprimee ailleurs pendant qu'on
        // etait hors ligne n'a jamais existe ici.
        db.appliquer_une_suppression("tache", "inconnu").unwrap();
    }
}
