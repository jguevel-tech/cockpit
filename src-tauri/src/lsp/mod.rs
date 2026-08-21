//! Client LSP minimal pour le "aller a la definition" du viewer de fichiers.
//! Un serveur par (racine projet, langage), spawn a la demande et garde vivant.
//! Une seule requete a la fois par serveur (Mutex global simple) — suffisant
//! pour un usage interactif. Seuls initialize / didOpen / didChange /
//! textDocument/definition sont utilises.

use crate::commande::SansConsole;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Delai max pour une definition : le premier appel paie l'indexation du
/// projet (intelephense/rust-analyzer), les suivants sont instantanes.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Serialize, Clone, Debug)]
pub struct DefLocation {
    pub rel_path: String,
    /// 0-indexes (convention LSP)
    pub line: u32,
    pub character: u32,
}

/// Binaire + args du serveur pour un language id LSP. Ajouter un langage = 1 ligne.
fn server_for(lang: &str) -> Option<(&'static str, &'static [&'static str])> {
    match lang {
        "php" => Some(("intelephense", &["--stdio"])),
        "rust" => Some(("rust-analyzer", &[])),
        "typescript" | "javascript" => Some(("typescript-language-server", &["--stdio"])),
        "svelte" => Some(("svelteserver", &["--stdio"])),
        "python" => Some(("pylsp", &[])),
        "go" => Some(("gopls", &[])),
    _ => None,
    }
}

fn binary_exists(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let p = dir.join(bin);
                p.is_file()
            })
        })
        .unwrap_or(false)
}

/// Un serveur LSP est-il installe pour ce langage ?
pub fn available(lang: &str) -> bool {
    server_for(lang).map(|(bin, _)| binary_exists(bin)).unwrap_or(false)
}

struct LspServer {
    child: Child,
    stdin: ChildStdin,
    /// Messages entrants parses par le thread lecteur.
    incoming: Receiver<Value>,
    next_id: i64,
    /// uri -> version envoyee (didOpen une fois, didChange ensuite)
    opened: HashMap<String, i32>,
}

#[derive(Default)]
pub struct LspState {
    /// cle : "<racine>|<lang>"
    servers: Mutex<HashMap<String, LspServer>>,
}

fn frame(msg: &Value) -> Vec<u8> {
    let body = msg.to_string();
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body).into_bytes()
}

/// Thread lecteur : parse le framing LSP et pousse chaque message JSON.
fn spawn_reader(stdout: impl Read + Send + 'static, tx: Sender<Value>) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) => {}
                }
                let line = line.trim_end();
                if line.is_empty() {
                    break;
                }
                if let Some(v) = line.strip_prefix("Content-Length:") {
                    content_length = v.trim().parse().unwrap_or(0);
                }
            }
            if content_length == 0 {
                continue;
            }
            let mut buf = vec![0u8; content_length];
            if reader.read_exact(&mut buf).is_err() {
                return;
            }
            if let Ok(msg) = serde_json::from_slice::<Value>(&buf) {
                if tx.send(msg).is_err() {
                    return;
                }
            }
        }
    });
}

impl LspServer {
    fn spawn(bin: &str, args: &[&str], root: &str) -> Result<Self, String> {
        let mut child = Command::new(bin)
            .sans_console()
            .args(args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("lancement {}: {}", bin, e))?;

        let stdin = child.stdin.take().ok_or("stdin indisponible")?;
        let stdout = child.stdout.take().ok_or("stdout indisponible")?;
        let (tx, incoming) = std::sync::mpsc::channel();
        spawn_reader(stdout, tx);

        let mut server = LspServer { child, stdin, incoming, next_id: 1, opened: HashMap::new() };

        let root_uri = format!("file://{}", root);
        server.request(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": root_uri,
                "workspaceFolders": [{ "uri": root_uri, "name": "project" }],
                "capabilities": {
                    "textDocument": { "definition": { "linkSupport": true } },
                    "workspace": { "workspaceFolders": true }
                }
            }),
            REQUEST_TIMEOUT,
        )?;
        server.notify("initialized", json!({}))?;
        Ok(server)
    }

    fn send_raw(&mut self, msg: &Value) -> Result<(), String> {
        self.stdin.write_all(&frame(msg)).map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        self.send_raw(&json!({ "jsonrpc": "2.0", "method": method, "params": params }))
    }

    /// Requete synchrone : attend la reponse a NOTRE id, repond null aux
    /// requetes serveur->client croisees (configuration, registerCapability...)
    /// pour ne pas bloquer les serveurs qui les attendent.
    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_raw(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;

        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("LSP: delai depasse (indexation en cours ? reessaie)")?;
            let msg = self
                .incoming
                .recv_timeout(remaining)
                .map_err(|_| "LSP: delai depasse (indexation en cours ? reessaie)".to_string())?;

            if msg.get("id").and_then(|i| i.as_i64()) == Some(id) && msg.get("method").is_none() {
                if let Some(err) = msg.get("error") {
                    return Err(format!("LSP: {}", err.get("message").and_then(|m| m.as_str()).unwrap_or("erreur")));
                }
                return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
            }
            // Requete du serveur vers nous : reponse neutre pour le debloquer
            if msg.get("method").is_some() && msg.get("id").is_some() {
                let reply_id = msg.get("id").cloned().unwrap_or(Value::Null);
                let _ = self.send_raw(&json!({ "jsonrpc": "2.0", "id": reply_id, "result": Value::Null }));
            }
            // Notifications (logs, diagnostics...) : ignorees
        }
    }

    fn ensure_open(&mut self, uri: &str, lang: &str, text: &str) -> Result<(), String> {
        match self.opened.get_mut(uri) {
            None => {
                self.opened.insert(uri.to_string(), 1);
                let (uri, lang, text) = (uri.to_string(), lang.to_string(), text.to_string());
                self.notify(
                    "textDocument/didOpen",
                    json!({ "textDocument": { "uri": uri, "languageId": lang, "version": 1, "text": text } }),
                )
            }
            Some(version) => {
                *version += 1;
                let v = *version;
                let uri = uri.to_string();
                let text = text.to_string();
                self.notify(
                    "textDocument/didChange",
                    json!({
                        "textDocument": { "uri": uri, "version": v },
                        "contentChanges": [{ "text": text }]
                    }),
                )
            }
        }
    }

    fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl LspState {
    /// textDocument/definition. `content` = texte courant du fichier (peut etre
    /// une version editee non sauvee). Positions 0-indexees.
    pub fn definition(
        &self,
        root: &str,
        lang: &str,
        rel_path: &str,
        content: &str,
        line: u32,
        character: u32,
    ) -> Result<Vec<DefLocation>, String> {
        let (bin, args) = server_for(lang).ok_or("pas de serveur LSP pour ce langage")?;
        if !binary_exists(bin) {
            return Err(format!("{} non installe", bin));
        }

        let key = format!("{}|{}", root, lang);
        let mut servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());

        // Serveur mort (crash, kill) -> on le relance
        if let Some(s) = servers.get_mut(&key) {
            if !s.is_alive() {
                servers.remove(&key);
            }
        }
        if !servers.contains_key(&key) {
            servers.insert(key.clone(), LspServer::spawn(bin, args, root)?);
        }
        let server = servers.get_mut(&key).unwrap();

        let uri = format!("file://{}/{}", root.trim_end_matches('/'), rel_path);
        server.ensure_open(&uri, lang, content)?;

        let result = server.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": line, "character": character }
            }),
            REQUEST_TIMEOUT,
        )?;

        Ok(parse_locations(&result, root))
    }

    /// Arret propre de tous les serveurs (fermeture de l'app).
    pub fn shutdown_all(&self) {
        let mut servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
        for (_, mut s) in servers.drain() {
            let _ = s.notify("exit", json!({}));
            let _ = s.child.kill();
        }
    }
}

/// Normalise Location | Location[] | LocationLink[] -> chemins RELATIFS au
/// projet (les resultats hors racine — stdlib, ~/.cargo — sont ecartes,
/// le viewer est verrouille a la racine).
fn parse_locations(result: &Value, root: &str) -> Vec<DefLocation> {
    let root_uri = format!("file://{}", root.trim_end_matches('/'));
    let items: Vec<&Value> = match result {
        Value::Array(a) => a.iter().collect(),
        Value::Object(_) => vec![result],
        _ => vec![],
    };

    items
        .iter()
        .filter_map(|item| {
            let (uri, range) = if item.get("targetUri").is_some() {
                // LocationLink : targetSelectionRange pointe le nom exact
                (
                    item.get("targetUri")?.as_str()?,
                    item.get("targetSelectionRange").or_else(|| item.get("targetRange"))?,
                )
            } else {
                (item.get("uri")?.as_str()?, item.get("range")?)
            };
            let path = uri.strip_prefix(&root_uri)?.trim_start_matches('/');
            let start = range.get("start")?;
            Some(DefLocation {
                rel_path: path.to_string(),
                line: start.get("line")?.as_u64()? as u32,
                character: start.get("character")?.as_u64().unwrap_or(0) as u32,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_location_array_and_links() {
        let root = "/proj";
        let loc = json!([{ "uri": "file:///proj/src/A.php", "range": { "start": { "line": 4, "character": 6 } } }]);
        let hits = parse_locations(&loc, root);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].rel_path, "src/A.php");
        assert_eq!(hits[0].line, 4);

        let link = json!([{
            "targetUri": "file:///proj/src/B.rs",
            "targetRange": { "start": { "line": 10, "character": 0 } },
            "targetSelectionRange": { "start": { "line": 10, "character": 3 } }
        }]);
        let hits = parse_locations(&link, root);
        assert_eq!(hits[0].character, 3);

        // Hors racine -> ecarte
        let outside = json!({ "uri": "file:///usr/lib/x.rs", "range": { "start": { "line": 0, "character": 0 } } });
        assert!(parse_locations(&outside, root).is_empty());

        assert!(parse_locations(&Value::Null, root).is_empty());
    }
}
