//! Identifiants globaux et journal des changements, pour la synchronisation.
//!
//! **Rien ici ne modifie une requete existante.** Les identifiants des tables sont des entiers
//! locaux : deux machines donneraient le meme a deux choses differentes. Plutot que d'ajouter
//! une colonne partout — et de devoir penser a la remplir dans chaque insertion deja ecrite —
//! une table de cote associe a chaque ligne un identifiant global, et des declencheurs SQLite
//! la tiennent a jour. Un oubli devient impossible : ce n'est plus a l'appelant d'y penser.
//!
//! Le journal marche pareil : chaque insertion, modification ou suppression y laisse une trace,
//! sans qu'aucun code metier soit au courant. **C'est ce qui permet a une suppression de
//! voyager** : sans trace, l'element reviendrait depuis l'autre machine au tour suivant.
//!
//! **La pause existe pour une raison precise.** Quand on applique ce qui vient du serveur, on
//! ECRIT dans ces memes tables : sans pause, les declencheurs marqueraient ces lignes comme
//! modifiees et on les renverrait aussitot. Deux machines se renverraient les memes donnees
//! sans fin.

use super::db::Database;
use rusqlite::{Connection, Result};

/// Les tables qui voyagent, et le nom sous lequel le serveur les connait.
///
/// Le nom est un contrat avec le serveur : le CHANGER couperait la correspondance avec ce qui
/// est deja synchronise, et l'autre machine recreerait tout en double.
///
/// Ce qui n'est PAS ici est deliberement local : les terminaux (leur etat vivant n'existe que
/// sur la machine), l'historique des commandes, les enregistrements de reunion (des fichiers
/// audio, pas des donnees), et les reglages (une cle d'API ou un fond d'ecran n'ont rien a
/// faire ailleurs).
pub const TYPES: &[(&str, &str)] = &[
    ("dossier_projet", "project_folders"),
    ("projet", "projects"),
    ("tache", "todos"),
    ("lien", "urls"),
    ("commande", "project_commands"),
    ("dossier_note", "note_folders"),
    ("fichier_note", "note_files"),
    ("note", "notes"),
];

/// Un UUID version 4 fabrique en SQL.
///
/// SQLite n'en fournit pas, et un declencheur ne peut pas appeler du Rust. Les deux constantes
/// imposees par la norme sont le « 4 » de la version et le premier caractere de la quatrieme
/// section, tire de « 89ab ».
const UUID_SQL: &str = "lower(\
    hex(randomblob(4)) || '-' || \
    hex(randomblob(2)) || '-4' || substr(hex(randomblob(2)), 2) || '-' || \
    substr('89ab', abs(random()) % 4 + 1, 1) || substr(hex(randomblob(2)), 2) || '-' || \
    hex(randomblob(6)))";

/// L'instant courant en millisecondes depuis l'epoque.
///
/// Les colonnes `created_at` des tables sont en secondes et en heure locale ; on ne s'en sert
/// pas. L'arbitrage entre deux machines se joue parfois a moins d'une seconde.
const MAINTENANT_SQL: &str = "CAST((julianday('now') - 2440587.5) * 86400000.0 AS INTEGER)";

/// Marque que le contenu deja present a ete mis en file, une fois pour toutes.
const CLE_AMORCE: &str = "sync_journal_amorce";

impl Database {
    /// Cree les tables et les declencheurs, et rattrape les lignes deja presentes.
    pub fn preparer_la_synchro(&self) -> Result<()> {
        let conn = self.conn();

        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS sync_identite (
                type     TEXT    NOT NULL,
                local_id INTEGER NOT NULL,
                uuid     TEXT    NOT NULL,
                PRIMARY KEY (type, local_id)
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_identite_uuid ON sync_identite(uuid);

            -- `rev` est un numero local qui augmente a CHAQUE ecriture du journal. Il sert a
            -- acquitter un envoi : effacer en comparant l'horodatage perdrait une retouche
            -- faite dans la meme milliseconde que l'envoi — silencieusement, et pour toujours.
            -- SQLite serialise les ecritures, donc MAX+1 ne peut pas rendre deux fois la meme.
            CREATE TABLE IF NOT EXISTS sync_journal (
                type        TEXT    NOT NULL,
                uuid        TEXT    NOT NULL,
                rev         INTEGER NOT NULL,
                modifie_le  INTEGER NOT NULL,
                supprime_le INTEGER DEFAULT NULL,
                PRIMARY KEY (type, uuid)
            );

            -- Une seule ligne, qui dit si les declencheurs doivent journaliser. Une table et
            -- non une variable : un declencheur ne peut lire que la base.
            CREATE TABLE IF NOT EXISTS sync_pause (actif INTEGER NOT NULL);
            INSERT INTO sync_pause (actif)
                SELECT 0 WHERE NOT EXISTS (SELECT 1 FROM sync_pause);

            -- Remise a zero a CHAQUE demarrage. La pause vit en base : un arret brutal
            -- en pleine application la laisserait active pour toujours, et plus rien ne
            -- partirait jamais — sans message, sans erreur, sans rien qui se voie.
            UPDATE sync_pause SET actif = 0;
            ",
        )?;

        for (type_, table) in TYPES {
            poser_les_declencheurs(&conn, type_, table)?;
            rattraper_les_lignes_existantes(&conn, type_, table)?;
        }
        drop(conn);

        // UNE SEULE FOIS PAR INSTALLATION. Le rattrapage ci-dessus donne une identite a ce qui
        // existait deja, mais ne met rien en file : une installation qui a servi avant cette
        // fonctionnalite n'envoyait donc jamais son contenu, et une seconde machine ne recevait
        // rien — pour toujours. Le drapeau evite de tout renvoyer a chaque demarrage.
        if self.get_setting(CLE_AMORCE).is_none() {
            let n = self.amorcer_le_journal()?;
            self.set_setting(CLE_AMORCE, "1").map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(e))
            })?;
            if n > 0 {
                log::info!("synchro : {n} elements deja presents mis en file d'envoi");
            }
        }

        Ok(())
    }

    /// Suspend la journalisation le temps d'appliquer ce qui vient du serveur.
    pub fn suspendre_la_journalisation(&self, actif: bool) -> Result<()> {
        self.conn()
            .execute("UPDATE sync_pause SET actif = ?1", [i32::from(actif)])?;
        Ok(())
    }

    /// L'identifiant global d'une ligne, s'il existe.
    pub fn uuid_de(&self, type_: &str, local_id: i64) -> Option<String> {
        self.conn()
            .query_row(
                "SELECT uuid FROM sync_identite WHERE type = ?1 AND local_id = ?2",
                rusqlite::params![type_, local_id],
                |l| l.get(0),
            )
            .ok()
    }

    /// L'identifiant local correspondant a un identifiant global, s'il existe ici.
    pub fn local_de(&self, type_: &str, uuid: &str) -> Option<i64> {
        self.conn()
            .query_row(
                "SELECT local_id FROM sync_identite WHERE type = ?1 AND uuid = ?2",
                rusqlite::params![type_, uuid],
                |l| l.get(0),
            )
            .ok()
    }

    /// Associe un identifiant global a une ligne, en remplacant une eventuelle association
    /// precedente pour cette ligne.
    pub fn associer_uuid(&self, type_: &str, local_id: i64, uuid: &str) -> Result<()> {
        self.conn().execute(
            "INSERT INTO sync_identite (type, local_id, uuid) VALUES (?1, ?2, ?3)
             ON CONFLICT(type, local_id) DO UPDATE SET uuid = excluded.uuid",
            rusqlite::params![type_, local_id, uuid],
        )?;
        Ok(())
    }

    /// Ce qui attend d'etre envoye.
    pub fn changements_en_attente(&self) -> Result<Vec<ChangementLocal>> {
        let conn = self.conn();
        let mut requete = conn.prepare(
            "SELECT type, uuid, rev, modifie_le, supprime_le FROM sync_journal ORDER BY rev",
        )?;
        let lignes = requete.query_map([], |l| {
            Ok(ChangementLocal {
                type_: l.get(0)?,
                uuid: l.get(1)?,
                rev: l.get(2)?,
                modifie_le: l.get(3)?,
                supprime_le: l.get(4)?,
            })
        })?;
        lignes.collect()
    }

    /// Oublie du journal ce qui a ete accepte par le serveur.
    ///
    /// La comparaison porte sur `rev` et NON sur l'horodatage : entre l'envoi et
    /// l'acquittement, l'utilisateur a pu retoucher la meme chose dans la meme milliseconde.
    /// Deux horodatages egaux se seraient effaces l'un l'autre et la retouche aurait disparu,
    /// sans trace. Un numero, lui, est strictement croissant.
    pub fn oublier_les_changements_envoyes(&self, envoyes: &[ChangementLocal]) -> Result<()> {
        let conn = self.conn();
        for c in envoyes {
            conn.execute(
                "DELETE FROM sync_journal WHERE type = ?1 AND uuid = ?2 AND rev <= ?3",
                rusqlite::params![c.type_, c.uuid, c.rev],
            )?;
        }
        Ok(())
    }

    /// Inscrit au journal une suppression venue du serveur, pour que la trace existe aussi ici.
    pub fn journaliser(&self, type_: &str, uuid: &str, modifie_le: i64, supprime_le: Option<i64>) -> Result<()> {
        self.conn().execute(
            "INSERT INTO sync_journal (type, uuid, rev, modifie_le, supprime_le)
             VALUES (?1, ?2, (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal), ?3, ?4)
             ON CONFLICT(type, uuid) DO UPDATE SET
                rev = (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal),
                modifie_le = excluded.modifie_le, supprime_le = excluded.supprime_le",
            rusqlite::params![type_, uuid, modifie_le, supprime_le],
        )?;
        Ok(())
    }

    /// Met en file d'envoi TOUT ce que cette machine possede deja.
    ///
    /// ## Pourquoi c'est indispensable
    ///
    /// Les declencheurs ne voient que ce qui CHANGE. Une installation qui a servi avant qu'un
    /// compte existe a donc une identite pour chaque element, et un journal vide : rien ne part
    /// jamais, et une seconde machine ne recoit rien — pour toujours, jusqu'a ce qu'on retouche
    /// chaque element a la main. Constate sur une vraie installation : 248 identites, 0 ligne de
    /// journal, 0 element sur le serveur.
    ///
    /// Appele quand une session s'ouvre, donc a la connexion comme a l'inscription.
    ///
    /// `ON CONFLICT DO NOTHING` : ce qui attend deja porte une revision plus fraiche, on n'y
    /// touche pas. Relancer cette fonction est donc sans effet, et c'est voulu — elle passe a
    /// chaque ouverture de session.
    pub fn amorcer_le_journal(&self) -> Result<usize> {
        Ok(self.conn().execute(
            "INSERT INTO sync_journal (type, uuid, rev, modifie_le, supprime_le)
                 SELECT i.type, i.uuid,
                        (SELECT IFNULL(MAX(rev), 0) FROM sync_journal)
                            + ROW_NUMBER() OVER (ORDER BY i.rowid),
                        CAST((julianday('now') - 2440587.5) * 86400000.0 AS INTEGER),
                        NULL
                 FROM sync_identite i
                 -- `WHERE true` n'est pas decoratif : sans lui SQLite ne sait pas si le
                 -- `ON CONFLICT` appartient au SELECT ou a l'INSERT, et refuse la requete.
                 WHERE true
             ON CONFLICT(type, uuid) DO NOTHING",
            [],
        )?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangementLocal {
    pub type_: String,
    pub uuid: String,
    /// Numero local du journal, strictement croissant. Sert a acquitter, jamais a arbitrer.
    pub rev: i64,
    pub modifie_le: i64,
    pub supprime_le: Option<i64>,
}

fn poser_les_declencheurs(conn: &Connection, type_: &str, table: &str) -> Result<()> {
    // L'identite est posee MEME en pause : appliquer une donnee venue du serveur cree bien une
    // ligne ici, et il faut pouvoir la retrouver. Seule la journalisation est suspendue.
    conn.execute_batch(&format!(
        "
        DROP TRIGGER IF EXISTS sync_{type_}_insert;
        CREATE TRIGGER sync_{type_}_insert AFTER INSERT ON {table}
        BEGIN
            INSERT OR IGNORE INTO sync_identite (type, local_id, uuid)
                VALUES ('{type_}', NEW.id, {UUID_SQL});

            INSERT INTO sync_journal (type, uuid, rev, modifie_le, supprime_le)
                SELECT '{type_}', i.uuid,
                       (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal),
                       {MAINTENANT_SQL}, NULL
                FROM sync_identite i
                WHERE i.type = '{type_}' AND i.local_id = NEW.id
                  AND (SELECT actif FROM sync_pause) = 0
            ON CONFLICT(type, uuid) DO UPDATE SET
                rev = (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal),
                modifie_le = excluded.modifie_le, supprime_le = NULL;
        END;

        DROP TRIGGER IF EXISTS sync_{type_}_update;
        CREATE TRIGGER sync_{type_}_update AFTER UPDATE ON {table}
        BEGIN
            INSERT INTO sync_journal (type, uuid, rev, modifie_le, supprime_le)
                SELECT '{type_}', i.uuid,
                       (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal),
                       {MAINTENANT_SQL}, NULL
                FROM sync_identite i
                WHERE i.type = '{type_}' AND i.local_id = NEW.id
                  AND (SELECT actif FROM sync_pause) = 0
            ON CONFLICT(type, uuid) DO UPDATE SET
                rev = (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal),
                modifie_le = excluded.modifie_le, supprime_le = NULL;
        END;

        -- La trace de suppression GARDE l'uuid apres que l'identite a disparu : c'est elle
        -- qui fera partir la suppression vers les autres machines.
        DROP TRIGGER IF EXISTS sync_{type_}_delete;
        CREATE TRIGGER sync_{type_}_delete AFTER DELETE ON {table}
        BEGIN
            INSERT INTO sync_journal (type, uuid, rev, modifie_le, supprime_le)
                SELECT '{type_}', i.uuid,
                       (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal),
                       {MAINTENANT_SQL}, {MAINTENANT_SQL}
                FROM sync_identite i
                WHERE i.type = '{type_}' AND i.local_id = OLD.id
                  AND (SELECT actif FROM sync_pause) = 0
            ON CONFLICT(type, uuid) DO UPDATE SET
                rev = (SELECT IFNULL(MAX(rev), 0) + 1 FROM sync_journal),
                modifie_le = excluded.modifie_le, supprime_le = excluded.supprime_le;

            DELETE FROM sync_identite WHERE type = '{type_}' AND local_id = OLD.id;
        END;
        "
    ))
}

/// Donne un identifiant global aux lignes deja presentes avant la mise en place.
///
/// Sans ce rattrapage, tout ce que l'utilisateur avait avant de creer un compte resterait
/// invisible du serveur : il verrait ses projets disparaitre en changeant de machine.
///
/// Ces lignes ne sont PAS journalisees ici : c'est la premiere synchronisation qui decide de
/// les envoyer, en une fois et volontairement.
fn rattraper_les_lignes_existantes(conn: &Connection, type_: &str, table: &str) -> Result<()> {
    conn.execute_batch(&format!(
        "INSERT OR IGNORE INTO sync_identite (type, local_id, uuid)
         SELECT '{type_}', t.id, {UUID_SQL} FROM {table} t
         WHERE NOT EXISTS (
             SELECT 1 FROM sync_identite i WHERE i.type = '{type_}' AND i.local_id = t.id
         );"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Database {
        let db = Database::new(":memory:").unwrap();
        db.preparer_la_synchro().unwrap();
        db
    }

    /// Le defaut qui a fait croire que la synchronisation ne marchait pas : une installation
    /// qui existait AVANT le compte n'envoyait jamais rien. Les declencheurs ne voient que ce
    /// qui change, et rien n'avait change depuis.
    #[test]
    fn ce_qui_existait_avant_le_compte_part_quand_meme() {
        let db = Database::new(":memory:").unwrap();

        // On se remet dans l'etat d'AVANT la synchronisation : plus de declencheurs, puis des
        // donnees. C'est l'histoire de toute installation qui a servi avant cette version.
        {
            let conn = db.conn();
            let noms: Vec<String> = conn
                .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'")
                .unwrap()
                .query_map([], |l| l.get(0))
                .unwrap()
                .collect::<Result<_>>()
                .unwrap();
            for nom in noms {
                conn.execute_batch(&format!("DROP TRIGGER {nom}")).unwrap();
            }
            conn.execute("INSERT INTO projects (name, path) VALUES ('avant', '/tmp/a')", [])
                .unwrap();
        }

        // La montee de version repose les declencheurs et rattrape les identites...
        db.preparer_la_synchro().unwrap();
        assert!(db.uuid_de("projet", 1).is_some(), "l'identite doit etre rattrapee");
        assert_eq!(
            db.changements_en_attente().unwrap().len(),
            0,
            "...mais elle ne met RIEN en file : c'est exactement le defaut constate"
        );

        let amorces = db.amorcer_le_journal().unwrap();
        assert!(amorces >= 1, "l'element existant devait etre mis en file");
        let attente = db.changements_en_attente().unwrap();
        assert_eq!(attente.len(), amorces, "tout ce qui a une identite doit attendre");
        assert!(attente.iter().all(|c| c.supprime_le.is_none()), "rien n'est supprime ici");
    }

    /// Le rattrapage doit se voir SANS y toucher : une base qui existait avant la
    /// synchronisation part en file toute seule au demarrage suivant.
    #[test]
    fn une_base_d_avant_la_synchro_se_met_en_file_au_demarrage() {
        let fichier = std::env::temp_dir().join(format!("ckpt-amorce-{}.db", std::process::id()));
        std::fs::remove_file(&fichier).ok();
        let chemin = fichier.to_string_lossy().to_string();

        {
            let db = Database::new(&chemin).unwrap();
            let conn = db.conn();
            let noms: Vec<String> = conn
                .prepare("SELECT name FROM sqlite_master WHERE type = 'trigger'")
                .unwrap()
                .query_map([], |l| l.get(0))
                .unwrap()
                .collect::<Result<_>>()
                .unwrap();
            for nom in noms {
                conn.execute_batch(&format!("DROP TRIGGER {nom}")).unwrap();
            }
            conn.execute("INSERT INTO projects (name, path) VALUES ('avant', '/tmp/a')", [])
                .unwrap();
            drop(conn);
            // On efface le drapeau : l'installation date d'avant qu'il existe.
            db.conn()
                .execute("DELETE FROM settings WHERE key = ?1", [CLE_AMORCE])
                .unwrap();
        }

        // Un demarrage suffit.
        let db = Database::new(&chemin).unwrap();
        assert_eq!(
            db.changements_en_attente().unwrap().len(),
            1,
            "le contenu d'avant doit etre en file des le demarrage"
        );

        // Et le demarrage SUIVANT ne renvoie pas tout une seconde fois.
        drop(db);
        let db = Database::new(&chemin).unwrap();
        assert_eq!(db.changements_en_attente().unwrap().len(), 1, "pas de second envoi");

        std::fs::remove_file(&fichier).ok();
    }

    /// L'amorcage passe a CHAQUE ouverture de session : il ne doit rien abimer.
    #[test]
    fn amorcer_deux_fois_ne_duplique_ni_ne_rajeunit_rien() {
        let db = base();
        creer_un_projet(&db, "deja-la");

        let avant = db.changements_en_attente().unwrap();
        assert_eq!(avant.len(), 1, "le declencheur a deja mis l'element en file");

        assert_eq!(db.amorcer_le_journal().unwrap(), 0, "rien de neuf a mettre en file");
        let apres = db.changements_en_attente().unwrap();
        assert_eq!(apres, avant, "ce qui attendait deja ne doit pas bouger");

        assert_eq!(db.amorcer_le_journal().unwrap(), 0);
        assert_eq!(db.changements_en_attente().unwrap(), avant);
    }

    fn creer_un_projet(db: &Database, nom: &str) -> i64 {
        let conn = db.conn();
        conn.execute(
            "INSERT INTO projects (name, path) VALUES (?1, '/tmp/x')",
            [nom],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn une_ligne_creee_recoit_un_identifiant_global_et_entre_au_journal() {
        let db = base();
        let id = creer_un_projet(&db, "essai");

        let uuid = db.uuid_de("projet", id).expect("identite posee par le declencheur");
        assert_eq!(uuid.len(), 36);

        let attente = db.changements_en_attente().unwrap();
        assert_eq!(attente.len(), 1);
        assert_eq!(attente[0].uuid, uuid);
        assert!(attente[0].supprime_le.is_none());
    }

    #[test]
    fn une_suppression_laisse_une_trace_qui_pourra_voyager() {
        let db = base();
        let id = creer_un_projet(&db, "a-supprimer");
        let uuid = db.uuid_de("projet", id).unwrap();

        db.conn().execute("DELETE FROM projects WHERE id = ?1", [id]).unwrap();

        // L'identite disparait — la ligne n'existe plus — mais la trace reste, sinon la
        // suppression ne partirait jamais et l'element reviendrait depuis l'autre machine.
        assert!(db.uuid_de("projet", id).is_none());
        let attente = db.changements_en_attente().unwrap();
        assert_eq!(attente.len(), 1);
        assert_eq!(attente[0].uuid, uuid);
        assert!(attente[0].supprime_le.is_some());
    }

    #[test]
    fn en_pause_rien_n_entre_au_journal_mais_l_identite_est_posee() {
        let db = base();
        db.suspendre_la_journalisation(true).unwrap();

        let id = creer_un_projet(&db, "venu-du-serveur");

        assert!(
            db.uuid_de("projet", id).is_some(),
            "sans identite, la ligne appliquee serait introuvable au prochain passage",
        );
        assert!(
            db.changements_en_attente().unwrap().is_empty(),
            "journaliser ce qu'on vient de recevoir le renverrait aussitot",
        );
    }

    #[test]
    fn les_lignes_deja_presentes_sont_rattrapees() {
        // Une base d'AVANT la synchronisation : on retire les declencheurs pour que la ligne
        // naisse comme elle naissait a l'epoque, sans identifiant global.
        let db = base();
        db.conn()
            .execute_batch("DROP TRIGGER sync_projet_insert; DELETE FROM sync_identite;")
            .unwrap();
        let id = creer_un_projet(&db, "d-avant");
        assert!(db.uuid_de("projet", id).is_none());

        db.preparer_la_synchro().unwrap();

        assert!(
            db.uuid_de("projet", id).is_some(),
            "sans rattrapage, tout ce qui existait avant le compte resterait invisible",
        );
        assert!(
            db.changements_en_attente().unwrap().is_empty(),
            "le rattrapage ne decide pas d'envoyer : c'est la premiere synchro qui le fait",
        );
    }

    #[test]
    fn preparer_deux_fois_ne_change_rien() {
        let db = base();
        let id = creer_un_projet(&db, "stable");
        let uuid = db.uuid_de("projet", id).unwrap();

        db.preparer_la_synchro().unwrap();

        assert_eq!(db.uuid_de("projet", id).unwrap(), uuid, "un identifiant ne doit jamais changer");
    }

    #[test]
    fn une_retouche_pendant_l_envoi_n_est_pas_perdue() {
        let db = base();
        let id = creer_un_projet(&db, "retouche");
        let envoye = db.changements_en_attente().unwrap();

        // L'utilisateur retouche pendant que l'envoi est en vol : le journal repasse a un
        // horodatage plus recent.
        db.conn()
            .execute("UPDATE projects SET description = 'apres' WHERE id = ?1", [id])
            .unwrap();

        db.oublier_les_changements_envoyes(&envoye).unwrap();

        assert_eq!(
            db.changements_en_attente().unwrap().len(),
            1,
            "effacer sans comparer l'horodatage perdrait la retouche, sans trace",
        );
    }
}
