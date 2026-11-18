use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;

        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        // Un panic dans un thread tenant le lock ne doit pas condamner tous les
        // acces DB suivants : on recupere le guard empoisonne (la connexion
        // SQLite reste valide, les transactions non commitees sont rollback).
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Reordonne des lignes par position a partir d'une liste d'IDs.
    /// `table` et `id_col` sont des noms de table/colonne connus (pas d'input utilisateur).
    pub fn reorder_by_ids(&self, table: &str, id_col: &str, ids: &[i64]) -> Result<(), String> {
        let conn = self.conn();
        let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
        let sql = format!("UPDATE {} SET position=?1 WHERE {}=?2", table, id_col);
        for (i, id) in ids.iter().enumerate() {
            tx.execute(&sql, rusqlite::params![i as i32, id])
                .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }

    /// Retourne la prochaine position disponible pour un insert.
    pub fn next_position(&self, table: &str, filter_col: &str, filter_val: &dyn rusqlite::types::ToSql) -> i32 {
        let conn = self.conn();
        let sql = format!(
            "SELECT COALESCE(MAX(position), -1) FROM {} WHERE {}=?1",
            table, filter_col
        );
        conn.query_row(&sql, [filter_val], |row| row.get::<_, i32>(0))
            .unwrap_or(-1)
            + 1
    }

    /// Variante pour filtre IS NULL (pas de valeur parente).
    pub fn next_position_null(&self, table: &str, filter_col: &str) -> i32 {
        let conn = self.conn();
        let sql = format!(
            "SELECT COALESCE(MAX(position), -1) FROM {} WHERE {} IS NULL",
            table, filter_col
        );
        conn.query_row(&sql, [], |row| row.get::<_, i32>(0))
            .unwrap_or(-1)
            + 1
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                path TEXT NOT NULL,
                compose_file TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                depends_on TEXT NOT NULL DEFAULT '[]',
                position INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS note_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                parent_id INTEGER DEFAULT NULL,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY (parent_id) REFERENCES note_folders(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS note_files (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                folder_id INTEGER DEFAULT NULL,
                name TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                position INTEGER NOT NULL DEFAULT 0,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (folder_id) REFERENCES note_folders(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                text TEXT NOT NULL,
                done INTEGER NOT NULL DEFAULT 0,
                position INTEGER NOT NULL DEFAULT 0,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS urls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project TEXT NOT NULL,
                label TEXT NOT NULL,
                url TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_notes_project ON notes(project);
            CREATE INDEX IF NOT EXISTS idx_note_folders_project ON note_folders(project);
            CREATE INDEX IF NOT EXISTS idx_note_files_project ON note_files(project);
            CREATE INDEX IF NOT EXISTS idx_note_files_folder ON note_files(folder_id);
            CREATE INDEX IF NOT EXISTS idx_todos_project ON todos(project);
            CREATE INDEX IF NOT EXISTS idx_urls_project ON urls(project);
            ",
        )?;

        // Migration: add position column if missing (same as Go)
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN position INTEGER NOT NULL DEFAULT 0", []);

        // Migration: project_folders table + folder_id on projects
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );",
        )?;
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN folder_id INTEGER DEFAULT NULL REFERENCES project_folders(id) ON DELETE SET NULL", []);
        let _ = conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_projects_folder ON projects(folder_id);");

        // Migration: imbrication sans limite des dossiers de projets (issue #2).
        // parent_id NULL = dossier de premier niveau. Pas de ON DELETE CASCADE : la
        // suppression d'un dossier n'est autorisee que s'il est VIDE (aucun projet, aucun
        // sous-dossier), donc il n'y a jamais d'enfant a emporter ni a remonter — et une
        // branche entiere ne peut pas disparaitre derriere un dossier replie.
        let _ = conn.execute(
            "ALTER TABLE project_folders ADD COLUMN parent_id INTEGER DEFAULT NULL REFERENCES project_folders(id) ON DELETE SET NULL",
            [],
        );
        let _ = conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_project_folders_parent ON project_folders(parent_id);");

        // Migration: drop removed AI orchestration tables if they exist.
        conn.execute_batch(
            "DROP TABLE IF EXISTS ticket_comments;
             DROP TABLE IF EXISTS tickets;
             DROP TABLE IF EXISTS team_templates;
             DROP TABLE IF EXISTS agent_roles;",
        )?;

        // Migration: drop the removed sitemap diff feature.
        conn.execute_batch("DROP TABLE IF EXISTS sitemap_pairs;")?;

        // Migration: settings key/value + recordings (enregistrement de reunions)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS recordings (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                project       TEXT NOT NULL,
                started_at    DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_secs INTEGER NOT NULL DEFAULT 0,
                state         TEXT NOT NULL DEFAULT 'recording',
                error         TEXT DEFAULT NULL,
                dir           TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_recordings_project ON recordings(project);",
        )?;
        // Migration: prompt de resume par projet (NULL = prompt global)
        let _ = conn.execute(
            "ALTER TABLE projects ADD COLUMN summary_prompt TEXT DEFAULT NULL",
            [],
        );
        // Migration: liaison d'un projet a Jira (cles de projets Jira, gabarit de branche).
        // NULL = non lie / gabarit par defaut. Pas dans `formes.rs` : non synchronise.
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN jira_cles TEXT DEFAULT NULL", []);
        let _ = conn.execute("ALTER TABLE projects ADD COLUMN jira_gabarit_branche TEXT DEFAULT NULL", []);

        // Migration: echeance optionnelle d'une tache (date ISO "2026-08-20", NULL = sans)
        let _ = conn.execute(
            "ALTER TABLE todos ADD COLUMN due_date TEXT DEFAULT NULL",
            [],
        );

        // Migration: avancement d'une tache, en pourcentage (0 = pas commencee, 100 = finie).
        // Demande de l'issue #15 : avec beaucoup de petites taches menees en parallele, savoir
        // lesquelles sont en cours et ou elles en sont.
        let _ = conn.execute(
            "ALTER TABLE todos ADD COLUMN progress INTEGER NOT NULL DEFAULT 0",
            [],
        );

        // Migration: commandes rapides par projet (boutons qui lancent une commande
        // dans un terminal Cockpit)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS project_commands (
                id       INTEGER PRIMARY KEY AUTOINCREMENT,
                project  TEXT NOT NULL,
                label    TEXT NOT NULL,
                command  TEXT NOT NULL,
                position INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_project_commands_project ON project_commands(project);",
        )?;

        // Migration: terminaux persistants. La ligne ne porte que ce qui doit survivre a un
        // redemarrage de la machine — le projet et le nom d'onglet ; l'etat vivant appartient
        // au service de terminaux, qui ne survit pas au redemarrage.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS terminals (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                project    TEXT NOT NULL,
                name       TEXT NOT NULL DEFAULT '',
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            );
            CREATE INDEX IF NOT EXISTS idx_terminals_project ON terminals(project);",
        )?;

        // Migration: de quoi retrouver un terminal apres l'extinction du poste.
        //
        // `cwd` est le dossier ou le shell a ete ouvert (un conteneur, un worktree, un
        // sous-dossier), sans quoi un terminal restaure repartirait a la racine du projet.
        // `snapshot` est la derniere photo de son ecran, telle que le service la rend : des
        // octets de terminal, pas du texte. Les deux restent LOCAUX — `terminals` n'est pas
        // dans les tables synchronisees, et une photo d'ecran n'a rien a faire sur un serveur.
        let _ = conn.execute("ALTER TABLE terminals ADD COLUMN cwd TEXT NOT NULL DEFAULT ''", []);
        let _ = conn.execute("ALTER TABLE terminals ADD COLUMN snapshot BLOB", []);
        let _ = conn.execute("ALTER TABLE terminals ADD COLUMN snapshot_at INTEGER", []);
        // `agent` : l'identifiant du fournisseur d'IA qui tournait dans ce terminal quand sa
        // photo a ete prise. Vide = un shell ordinaire. C'est ce qui permet de rouvrir la
        // conversation en cours et pas seulement un shell nu.
        let _ = conn.execute("ALTER TABLE terminals ADD COLUMN agent TEXT NOT NULL DEFAULT ''", []);

        // Migration : l'ordre des terminaux dans la barre laterale, choisi a la souris.
        //
        // **LES LIGNES DEJA LA PRENNENT L'ORDRE QU'ELLES AVAIENT**, c'est-a-dire celui de leur
        // identifiant. Un `DEFAULT 0` sans ce rattrapage mettrait tout le monde a egalite, et
        // l'ordre affiche changerait au premier reordonnancement d'un seul element.
        if conn
            .execute("ALTER TABLE terminals ADD COLUMN position INTEGER NOT NULL DEFAULT 0", [])
            .is_ok()
        {
            let _ = conn.execute(
                "UPDATE terminals SET position = (
                     SELECT COUNT(*) FROM terminals AS avant WHERE avant.id < terminals.id
                 )",
                [],
            );
        }

        // Migration: le nom de session tmux n'existe plus (chantier des terminaux, aout 2026).
        // Tolere : sur un SQLite anterieur a 3.35 le DROP COLUMN echoue, et la colonne reste
        // avec sa valeur par defaut — plus personne ne la lit ni ne l'ecrit.
        let _ = conn.execute("ALTER TABLE terminals DROP COLUMN tmux_name", []);

        // Migration: historique de commandes (autosuggestion + Ctrl+R)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS command_history (
                command TEXT PRIMARY KEY,
                project TEXT NOT NULL,
                ts      INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_command_history_ts ON command_history(ts);",
        )?;

        // Migration: l'historique des mesures d'un namespace Kubernetes.
        //
        // **LOCALE ET BORNEE.** Elle n'est pas dans `synchro::TYPES`, donc elle ne voyage pas :
        // ces points ne decrivent que ce que CETTE machine a observe, et ils se comptent en
        // dizaines de milliers par jour. `WITHOUT ROWID` parce que la cle est deja la ligne
        // entiere ou presque : SQLite s'epargne un index de plus.
        // **LE NOM DU POD EST RANGE UNE FOIS, PAS A CHAQUE POINT.** Mesure faite avant de
        // choisir : une table plate ou chaque ligne repete cluster, namespace et nom de pod
        // pese 160 octets par point, soit 5,5 Mo par jour et par namespace. Avec une table
        // d'identifiants, le meme jour tient dans 0,7 Mo — 22 octets par point. C'est ce qui
        // rend une retention de plusieurs jours tenable sur plusieurs namespaces.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS k8s_cibles (
                id        INTEGER PRIMARY KEY,
                contexte  TEXT NOT NULL,
                namespace TEXT NOT NULL,
                pod       TEXT NOT NULL,
                UNIQUE (contexte, namespace, pod)
            );
            CREATE TABLE IF NOT EXISTS k8s_mesures (
                cible INTEGER NOT NULL,
                t     INTEGER NOT NULL,
                cpu   INTEGER NOT NULL,
                ram   INTEGER NOT NULL,
                PRIMARY KEY (cible, t)
            ) WITHOUT ROWID;
            CREATE INDEX IF NOT EXISTS idx_k8s_mesures_t ON k8s_mesures(t);",
        )?;

        // La synchronisation entre machines est RETIREE (1.5.0), avec le serveur. Une base qui
        // l'a connue garde des declencheurs qui ecriraient dans un journal a chaque
        // modification, pour personne : on les retire, avec leurs tables et ce que le compte
        // avait range dans les reglages (dont son jeton). Sans effet sur une base neuve.
        retirer_la_synchro(&conn)?;

        // Migration: noms personnalises des conversations d'un agent, PAR FOURNISSEUR.
        //
        // La table d'avant (`claude_session_names`) n'avait qu'un identifiant pour cle, parce
        // qu'un seul produit existait. Un identifiant de conversation n'a de sens que chez son
        // fournisseur — un UUID pour l'un, un numero pour l'autre — donc deux conversations
        // differentes pouvaient porter le meme et echanger leurs noms. Les lignes existantes
        // sont recopiees comme etant celles de Claude, puis l'ancienne table part : sans la
        // recopie, les noms poses a la main disparaitraient sans un mot.
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS noms_conversations (
                fournisseur     TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                nom             TEXT NOT NULL,
                PRIMARY KEY (fournisseur, conversation_id)
            );",
        )?;
        // L'ancienne table n'existe que sur une installation deja en service. Son existence se
        // verifie ICI et non dans le SQL : une requete qui NOMME une table absente echoue des sa
        // preparation, quelle que soit la garde qu'on lui ajoute.
        let ancienne = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='claude_session_names'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if ancienne {
            // Une seule transaction : un arret entre la recopie et la suppression perdrait les
            // noms poses a la main.
            conn.execute_batch(
                "BEGIN;
                 INSERT OR IGNORE INTO noms_conversations (fournisseur, conversation_id, nom)
                     SELECT 'claude', session_id, name FROM claude_session_names;
                 DROP TABLE claude_session_names;
                 COMMIT;",
            )?;
        }

        Ok(())
    }
}

/// Les types que la synchronisation suivait : chacun avait trois declencheurs
/// `sync_<type>_{insert,update,delete}`. Liste FIGEE : c'est ce qui a ete pose chez les gens.
const TYPES_SYNCHRO_RETIRES: &[&str] = &[
    "dossier_projet", "projet", "tache", "lien", "commande", "dossier_note", "fichier_note", "note",
];

/// Retire ce que la synchronisation avait pose dans une base. Rejouable sans effet.
fn retirer_la_synchro(conn: &Connection) -> Result<()> {
    let mut sql = String::new();
    for type_ in TYPES_SYNCHRO_RETIRES {
        for quand in ["insert", "update", "delete"] {
            sql.push_str(&format!("DROP TRIGGER IF EXISTS sync_{type_}_{quand};\n"));
        }
    }
    sql.push_str(
        "DROP TABLE IF EXISTS sync_identite;
         DROP TABLE IF EXISTS sync_journal;
         DROP TABLE IF EXISTS sync_pause;
         DELETE FROM settings WHERE key LIKE 'compte\\_%' ESCAPE '\\'
            OR key IN ('sync_journal_amorce', 'error_reporting', 'error_reporting_user');",
    );
    conn.execute_batch(&sql)
}

impl Database {
    /// Sauvegarde la base VIVANTE vers `dest` via l'API backup de SQLite : coherent
    /// meme en mode WAL et pendant des ecritures, contrairement a une copie de fichier.
    pub fn backup_to(&self, dest: &str) -> Result<(), String> {
        let conn = self.conn();
        let mut dst = rusqlite::Connection::open(dest).map_err(|e| e.to_string())?;
        let backup = rusqlite::backup::Backup::new(&conn, &mut dst).map_err(|e| e.to_string())?;
        backup
            .run_to_completion(64, std::time::Duration::from_millis(20), None)
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **UNE BASE QUI A CONNU LA SYNCHRO EN SORT PROPRE.** Ses declencheurs ecriraient sinon
    /// dans un journal a chaque modification, et le jeton du compte resterait en base.
    #[test]
    fn la_synchro_d_une_ancienne_base_est_retiree() {
        let chemin = std::env::temp_dir().join(format!("cockpit_sans_synchro_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&chemin);
        {
            let db = Database::new(chemin.to_str().unwrap()).unwrap();
            let conn = db.conn();
            conn.execute_batch(
                "CREATE TABLE sync_journal (type TEXT);
                 CREATE TABLE sync_identite (type TEXT);
                 CREATE TABLE sync_pause (actif INTEGER);
                 CREATE TRIGGER sync_projet_insert AFTER INSERT ON projects
                   BEGIN INSERT INTO sync_journal VALUES ('projet'); END;
                 CREATE TRIGGER sync_note_delete AFTER DELETE ON notes
                   BEGIN INSERT INTO sync_journal VALUES ('note'); END;
                 INSERT INTO settings (key, value) VALUES ('compte_jeton', 'secret'),
                   ('compte_synchro_curseur', '12'), ('error_reporting', 'on'),
                   ('compter_ceci', 'garde'), ('langue', 'fr');",
            )
            .unwrap();
        }
        let db = Database::new(chemin.to_str().unwrap()).unwrap();
        let restes: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'sync%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(restes, 0, "declencheurs ou tables de synchro encore presents");
        assert_eq!(db.get_setting("compte_jeton"), None, "le jeton du compte est reste en base");
        assert_eq!(db.get_setting("compte_synchro_curseur"), None);
        assert_eq!(db.get_setting("error_reporting"), None);
        // Le motif ne mord pas a cote : `compter_` n'est pas `compte_`.
        assert_eq!(db.get_setting("compter_ceci").as_deref(), Some("garde"));
        assert_eq!(db.get_setting("langue").as_deref(), Some("fr"));
        let _ = std::fs::remove_file(&chemin);
    }

    #[test]
    fn test_backup_to() {
        let db = Database::new(":memory:").unwrap();
        db.create_todo("proj", "sauvegarde-moi").unwrap();

        let dest = std::env::temp_dir().join(format!("cockpit_backup_{}.db", std::process::id()));
        let _ = std::fs::remove_file(&dest);
        db.backup_to(dest.to_str().unwrap()).unwrap();

        let copy = Database::new(dest.to_str().unwrap()).unwrap();
        let todos = copy.get_todos("proj").unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].text, "sauvegarde-moi");

        let _ = std::fs::remove_file(&dest);
    }

    /// **LES NOMS POSES A LA MAIN SURVIVENT A LA MIGRATION.** L'ancienne table n'avait qu'un
    /// identifiant pour cle, parce qu'un seul fournisseur existait ; la nouvelle porte aussi le
    /// fournisseur. Sans recopie, tous les noms de conversations disparaitraient a la mise a
    /// jour, sans un mot — c'est la seule donnee de cette table.
    #[test]
    fn la_migration_des_noms_de_conversations_ne_perd_rien() {
        let db = Database::new(":memory:").unwrap();
        {
            let conn = db.conn();
            // On refabrique l'etat d'avant : l'ancienne table, avec un nom dedans.
            conn.execute_batch(
                "DROP TABLE IF EXISTS noms_conversations;
                 CREATE TABLE claude_session_names (
                    session_id TEXT PRIMARY KEY,
                    name       TEXT NOT NULL
                 );
                 INSERT INTO claude_session_names VALUES ('abc-123', 'audit du forum');",
            )
            .unwrap();
        }

        db.migrate().unwrap();

        let conn = db.conn();
        let nom: String = conn
            .query_row(
                "SELECT nom FROM noms_conversations WHERE fournisseur='claude' AND conversation_id='abc-123'",
                [],
                |l| l.get(0),
            )
            .expect("le nom doit avoir ete recopie");
        assert_eq!(nom, "audit du forum");

        // Et l'ancienne table s'en va : deux verites pour une meme chaine finiraient par
        // diverger.
        let reste: bool = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='claude_session_names'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(!reste, "l'ancienne table devait etre supprimee");
    }

    #[test]
    fn test_database_init_and_migrate() {
        let db = Database::new(":memory:").unwrap();
        let conn = db.conn();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"projects".to_string()));
        assert!(tables.contains(&"notes".to_string()));
        assert!(tables.contains(&"note_folders".to_string()));
        assert!(tables.contains(&"note_files".to_string()));
        assert!(tables.contains(&"todos".to_string()));
        assert!(tables.contains(&"urls".to_string()));
    }
}
