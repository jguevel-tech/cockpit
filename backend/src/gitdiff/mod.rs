//! Vue Git : status du repo + diffs unified parses en structures rendables.
//! Meme approche que Warp : shell-out vers git (pas de libgit2), parser maison,
//! modele FileDiff/Hunk/Line avec les deux numeros de ligne (old/new).

use serde::Serialize;
use tokio::process::Command;

pub mod worktree;

use crate::commande::SansConsole;

#[derive(Serialize, Clone)]
pub struct GitStatus {
    pub branch: String,
    pub is_repo: bool,
    pub files: Vec<StatusEntry>,
    /// Commits d'avance / de retard sur l'upstream (None si pas d'upstream).
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    pub has_upstream: bool,
    /// Totaux de lignes sur l'ensemble des fichiers modifies.
    pub total_additions: usize,
    pub total_deletions: usize,
}

#[derive(Serialize, Clone)]
pub struct StatusEntry {
    pub path: String,
    /// Code court affichable : "M", "A", "D", "R", "??", "U"...
    pub status: String,
    pub untracked: bool,
    /// true si le fichier a des modifs indexees (colonne X du porcelain).
    pub staged: bool,
    /// true si le fichier a des modifs non indexees (colonne Y, ou untracked).
    pub unstaged: bool,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Serialize, Clone)]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
}

#[derive(Serialize, Clone)]
pub struct FileDiff {
    pub path: String,
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Serialize, Clone)]
pub struct DiffHunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize, Clone)]
pub struct DiffLine {
    pub kind: String, // "add" | "del" | "context"
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: String,
}

/// Lance git dans le repo. Les codes retour 0 et 1 sont acceptes
/// (1 = "des differences existent" pour git diff).
pub(super) async fn run_git(repo: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .sans_console()
        .args(args)
        .current_dir(repo)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("git: {}", e))?;

    let code = output.status.code().unwrap_or(-1);
    if code != 0 && code != 1 {
        return Err(format!(
            "git {} (code {}): {}",
            args.first().unwrap_or(&""),
            code,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Runner strict pour les operations (add/commit/push/branch) : tout code != 0
/// est une erreur, avec stderr+stdout comme message (git ecrit sur les deux).
pub(super) async fn run_git_strict(repo: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .sans_console()
        .args(args)
        .current_dir(repo)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| format!("git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let msg = if !stderr.trim().is_empty() { stderr } else { stdout };
        return Err(msg.trim().chars().take(400).collect());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub async fn git_status(repo: &str) -> Result<GitStatus, String> {
    // Repo git ?
    let check = Command::new("git")
        .sans_console()
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(repo)
        .output()
        .await
        .map_err(|e| format!("git: {}", e))?;
    if !check.status.success() {
        return Ok(GitStatus {
            branch: String::new(),
            is_repo: false,
            files: vec![],
            ahead: None,
            behind: None,
            has_upstream: false,
            total_additions: 0,
            total_deletions: 0,
        });
    }

    let branch = run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
        .await
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // Compteurs +/- par fichier (tracked). --numstat en un seul appel.
    // On additionne staged (--cached) et unstaged pour un total par fichier.
    let mut stats: std::collections::HashMap<String, (usize, usize)> = std::collections::HashMap::new();
    for args in [
        ["diff", "--numstat", "--no-color", "-z"].as_slice(),
        ["diff", "--numstat", "--no-color", "-z", "--cached"].as_slice(),
    ] {
        if let Ok(out) = run_git(repo, args).await {
            merge_numstat(&out, &mut stats);
        }
    }

    let raw = run_git(repo, &["status", "--porcelain", "-z"]).await?;
    let mut files = Vec::new();
    let mut parts = raw.split('\0').peekable();
    while let Some(entry) = parts.next() {
        if entry.len() < 4 {
            continue;
        }
        let xy = &entry[..2];
        let path = entry[3..].to_string();
        // Renommage : "R  new" suivi de l'ancien chemin dans le champ suivant
        if xy.starts_with('R') || xy.starts_with('C') {
            let _old = parts.next();
        }
        let untracked = xy == "??";
        let x = xy.chars().next().unwrap();
        let y = xy.chars().nth(1).unwrap();
        let staged = !untracked && x != ' ' && x != '?';
        let unstaged = untracked || (y != ' ');
        let status = if untracked {
            "??".to_string()
        } else {
            (if x != ' ' { x } else { y }).to_string()
        };
        let (additions, deletions) = stats.get(&path).copied().unwrap_or((0, 0));
        files.push(StatusEntry { path, status, untracked, staged, unstaged, additions, deletions });
    }

    // Untracked : compter les lignes via diff --no-index contre /dev/null
    for f in files.iter_mut().filter(|f| f.untracked && f.additions == 0) {
        if let Ok(raw) = run_git(repo, &["diff", "--numstat", "--no-color", "-z", "--no-index", "--", "/dev/null", &f.path]).await {
            let mut one = std::collections::HashMap::new();
            merge_numstat(&raw, &mut one);
            if let Some((a, d)) = one.values().next() {
                f.additions = *a;
                f.deletions = *d;
            }
        }
    }

    let total_additions = files.iter().map(|f| f.additions).sum();
    let total_deletions = files.iter().map(|f| f.deletions).sum();

    // Ahead / behind vs upstream
    let (mut ahead, mut behind, mut has_upstream) = (None, None, false);
    if let Ok(counts) = run_git(repo, &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"]).await {
        let nums: Vec<u32> = counts.split_whitespace().filter_map(|n| n.parse().ok()).collect();
        if nums.len() == 2 {
            has_upstream = true;
            behind = Some(nums[0]);
            ahead = Some(nums[1]);
        }
    }

    Ok(GitStatus {
        branch,
        is_repo: true,
        files,
        ahead,
        behind,
        has_upstream,
        total_additions,
        total_deletions,
    })
}

/// Parse une sortie `git diff --numstat -z` : `additions\tdeletions\tpath\0`.
fn merge_numstat(raw: &str, out: &mut std::collections::HashMap<String, (usize, usize)>) {
    for rec in raw.split('\0') {
        let cols: Vec<&str> = rec.splitn(3, '\t').collect();
        if cols.len() != 3 {
            continue;
        }
        // "-" pour les binaires
        let a = cols[0].parse::<usize>().unwrap_or(0);
        let d = cols[1].parse::<usize>().unwrap_or(0);
        let path = cols[2].to_string();
        let e = out.entry(path).or_insert((0, 0));
        e.0 += a;
        e.1 += d;
    }
}

// --- Operations git (staging, commit, push, branches) ---

pub async fn git_stage(repo: &str, path: &str) -> Result<(), String> {
    run_git_strict(repo, &["add", "--", path]).await.map(|_| ())
}

pub async fn git_unstage(repo: &str, path: &str) -> Result<(), String> {
    // reset restaure l'index depuis HEAD pour ce fichier (marche aussi si pas de HEAD)
    run_git_strict(repo, &["reset", "-q", "HEAD", "--", path]).await.map(|_| ())
}

pub async fn git_stage_all(repo: &str) -> Result<(), String> {
    run_git_strict(repo, &["add", "-A"]).await.map(|_| ())
}

pub async fn git_unstage_all(repo: &str) -> Result<(), String> {
    run_git_strict(repo, &["reset", "-q"]).await.map(|_| ())
}

pub async fn git_commit(repo: &str, message: &str) -> Result<(), String> {
    if message.trim().is_empty() {
        return Err("message de commit vide".into());
    }
    run_git_strict(repo, &["commit", "-m", message]).await.map(|_| ())
}

pub async fn git_push(repo: &str, set_upstream: bool) -> Result<String, String> {
    let branch = run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]).await?.trim().to_string();
    let args: Vec<&str> = if set_upstream {
        vec!["push", "--set-upstream", "origin", &branch]
    } else {
        vec!["push"]
    };
    run_git_strict(repo, &args).await
}

pub async fn git_branches(repo: &str) -> Result<Vec<BranchInfo>, String> {
    let raw = run_git(repo, &["branch", "--format=%(HEAD)%(refname:short)"]).await?;
    Ok(raw
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.is_empty() {
                return None;
            }
            let current = l.starts_with('*');
            let name = l.trim_start_matches('*').to_string();
            if name.is_empty() {
                None
            } else {
                Some(BranchInfo { name, current })
            }
        })
        .collect())
}

pub async fn git_checkout_branch(repo: &str, name: &str) -> Result<(), String> {
    run_git_strict(repo, &["checkout", name]).await.map(|_| ())
}

pub async fn git_create_branch(repo: &str, name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("nom de branche vide".into());
    }
    run_git_strict(repo, &["checkout", "-b", name.trim()]).await.map(|_| ())
}

pub async fn git_delete_branch(repo: &str, name: &str, force: bool) -> Result<(), String> {
    let flag = if force { "-D" } else { "-d" };
    run_git_strict(repo, &["branch", flag, name]).await.map(|_| ())
}

#[derive(Serialize, Clone)]
pub struct CommitInfo {
    pub hash: String,
    pub full_hash: String,
    pub author: String,
    /// Epoch (secondes) du commit : le frontend formate en relatif, sans souci de locale.
    pub epoch: i64,
    /// Decorations ("HEAD -> main, tag: v1.0", vide sinon).
    pub refs: String,
    pub subject: String,
}

/// Historique des commits de la branche courante (le plus recent d'abord).
pub async fn git_log(repo: &str, limit: u32) -> Result<Vec<CommitInfo>, String> {
    let n = format!("-{}", limit.clamp(1, 500));
    // %x1f (unit separator) : aucun risque de collision avec un sujet de commit
    let raw = match run_git_strict(repo, &["log", &n, "--pretty=format:%h%x1f%H%x1f%an%x1f%ct%x1f%D%x1f%s"]).await {
        Ok(r) => r,
        // Repo tout neuf sans aucun commit : un historique vide, pas une erreur
        Err(e) if e.contains("does not have any commits") => return Ok(vec![]),
        Err(e) => return Err(e),
    };
    Ok(parse_git_log(&raw))
}

fn parse_git_log(raw: &str) -> Vec<CommitInfo> {
    raw.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\u{1f}').collect();
            if parts.len() != 6 {
                return None;
            }
            Some(CommitInfo {
                hash: parts[0].to_string(),
                full_hash: parts[1].to_string(),
                author: parts[2].to_string(),
                epoch: parts[3].parse().unwrap_or(0),
                refs: parts[4].to_string(),
                subject: parts[5].to_string(),
            })
        })
        .collect()
}

/// Diff complet d'un commit (tous fichiers), parse avec le meme automate que les diffs
/// de l'arbre de travail. `--format=` vide le message pour ne garder que le diff.
pub async fn git_commit_diff(repo: &str, hash: &str) -> Result<Vec<FileDiff>, String> {
    // Un hash est hexadecimal : tout autre contenu serait une option ou une ref arbitraire
    if hash.is_empty() || hash.len() > 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("hash invalide".into());
    }
    let raw = run_git_strict(repo, &["show", "--no-color", "--no-ext-diff", "--format=", hash]).await?;
    Ok(split_multi_file_diff(&raw))
}

/// Decoupe la sortie multi-fichiers de `git show`/`git diff` sur les en-tetes
/// "diff --git a/x b/y", puis parse chaque bloc avec parse_unified_diff.
fn split_multi_file_diff(raw: &str) -> Vec<FileDiff> {
    fn flush(path: Option<String>, buf: &str, out: &mut Vec<FileDiff>) {
        let Some(p) = path else { return };
        let hunks = parse_unified_diff(buf);
        let additions = hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == "add").count();
        let deletions = hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == "del").count();
        out.push(FileDiff { path: p, hunks, additions, deletions });
    }

    let mut out = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut buf = String::new();
    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            flush(cur_path.take(), &buf, &mut out);
            buf.clear();
            cur_path = line.rfind(" b/").map(|i| line[i + 3..].to_string());
        } else if cur_path.is_some() {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    flush(cur_path.take(), &buf, &mut out);
    out
}

/// Pull en --ff-only : JAMAIS de merge ou rebase surprise depuis un bouton d'UI.
/// En cas de divergence, git repond par un message clair plutot que de laisser
/// un etat a moitie merge que l'utilisateur n'a pas demande.
pub async fn git_pull(repo: &str) -> Result<String, String> {
    run_git_strict(repo, &["pull", "--ff-only"]).await
}

pub async fn git_diff_file(repo: &str, path: &str, untracked: bool) -> Result<FileDiff, String> {
    let raw = if untracked {
        run_git(repo, &["diff", "--no-color", "--no-ext-diff", "--no-index", "--", "/dev/null", path]).await?
    } else {
        run_git(repo, &["diff", "--no-color", "--no-ext-diff", "HEAD", "--", path]).await?
    };

    let hunks = parse_unified_diff(&raw);
    let additions = hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == "add").count();
    let deletions = hunks.iter().flat_map(|h| &h.lines).filter(|l| l.kind == "del").count();

    Ok(FileDiff { path: path.to_string(), hunks, additions, deletions })
}

/// Parse un unified diff (sortie de `git diff`) en hunks/lignes.
/// Automate ligne a ligne, comme le parser de Warp/GitHub Desktop.
fn parse_unified_diff(raw: &str) -> Vec<DiffHunk> {
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut current: Option<(DiffHunk, u32, u32)> = None; // (hunk, old_no, new_no)

    for line in raw.lines() {
        if line.starts_with("@@") {
            if let Some((hunk, _, _)) = current.take() {
                hunks.push(hunk);
            }
            let (old_start, new_start) = parse_hunk_header(line).unwrap_or((1, 1));
            current = Some((
                DiffHunk { header: line.to_string(), lines: Vec::new() },
                old_start,
                new_start,
            ));
            continue;
        }

        let Some((hunk, old_no, new_no)) = current.as_mut() else { continue };

        if let Some(text) = line.strip_prefix('+') {
            hunk.lines.push(DiffLine {
                kind: "add".into(),
                old_line: None,
                new_line: Some(*new_no),
                text: text.to_string(),
            });
            *new_no += 1;
        } else if let Some(text) = line.strip_prefix('-') {
            hunk.lines.push(DiffLine {
                kind: "del".into(),
                old_line: Some(*old_no),
                new_line: None,
                text: text.to_string(),
            });
            *old_no += 1;
        } else if line.starts_with('\\') {
            // "\ No newline at end of file" : ignore
        } else {
            let text = line.strip_prefix(' ').unwrap_or(line);
            hunk.lines.push(DiffLine {
                kind: "context".into(),
                old_line: Some(*old_no),
                new_line: Some(*new_no),
                text: text.to_string(),
            });
            *old_no += 1;
            *new_no += 1;
        }
    }

    if let Some((hunk, _, _)) = current.take() {
        hunks.push(hunk);
    }
    hunks
}

/// Extrait les lignes de depart old/new d'un header `@@ -12,5 +14,7 @@ ...`.
fn parse_hunk_header(header: &str) -> Option<(u32, u32)> {
    let mut old_start = None;
    let mut new_start = None;
    for token in header.split_whitespace() {
        if let Some(rest) = token.strip_prefix('-') {
            old_start = rest.split(',').next()?.parse().ok();
        } else if let Some(rest) = token.strip_prefix('+') {
            new_start = rest.split(',').next()?.parse().ok();
        }
    }
    Some((old_start?, new_start?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/f.txt b/f.txt
index 1234567..89abcde 100644
--- a/f.txt
+++ b/f.txt
@@ -1,4 +1,4 @@
 ligne un
-ligne deux
+ligne 2
 ligne trois
@@ -10,2 +10,3 @@ contexte
 fin
+ajout
\\ No newline at end of file
";

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -12,5 +14,7 @@ fn main()"), Some((12, 14)));
        assert_eq!(parse_hunk_header("@@ -1 +1,2 @@"), Some((1, 1)));
    }

    #[test]
    fn test_parse_unified_diff() {
        let hunks = parse_unified_diff(SAMPLE);
        assert_eq!(hunks.len(), 2);

        let h1 = &hunks[0];
        assert_eq!(h1.lines.len(), 4);
        assert_eq!(h1.lines[0].kind, "context");
        assert_eq!(h1.lines[0].old_line, Some(1));
        assert_eq!(h1.lines[0].new_line, Some(1));
        assert_eq!(h1.lines[1].kind, "del");
        assert_eq!(h1.lines[1].old_line, Some(2));
        assert_eq!(h1.lines[1].new_line, None);
        assert_eq!(h1.lines[2].kind, "add");
        assert_eq!(h1.lines[2].new_line, Some(2));
        assert_eq!(h1.lines[3].old_line, Some(3));
        assert_eq!(h1.lines[3].new_line, Some(3));

        let h2 = &hunks[1];
        assert_eq!(h2.lines.len(), 2);
        assert_eq!(h2.lines[1].kind, "add");
        assert_eq!(h2.lines[1].new_line, Some(11));
    }

    #[test]
    fn test_parse_git_log() {
        let raw = "abc123\u{1f}abc123def\u{1f}le mainteneur\u{1f}1755000000\u{1f}HEAD -> main\u{1f}Corriger le bug\n\
                   def456\u{1f}def456abc\u{1f}Alice\u{1f}1754000000\u{1f}\u{1f}Sujet avec des \u{1f} impossibles ? non";
        let commits = parse_git_log(raw);
        assert_eq!(commits.len(), 1, "la 2e ligne a 7 champs (separateur dans le sujet simule) et doit etre ignoree");
        assert_eq!(commits[0].hash, "abc123");
        assert_eq!(commits[0].author, "le mainteneur");
        assert_eq!(commits[0].epoch, 1755000000);
        assert_eq!(commits[0].refs, "HEAD -> main");
        assert_eq!(commits[0].subject, "Corriger le bug");
    }

    #[test]
    fn test_split_multi_file_diff() {
        let raw = "diff --git a/src/a.rs b/src/a.rs\n\
                   index 111..222 100644\n\
                   --- a/src/a.rs\n\
                   +++ b/src/a.rs\n\
                   @@ -1,2 +1,2 @@\n\
                    contexte\n\
                   -ancien\n\
                   +nouveau\n\
                   diff --git a/b.txt b/b.txt\n\
                   new file mode 100644\n\
                   --- /dev/null\n\
                   +++ b/b.txt\n\
                   @@ -0,0 +1 @@\n\
                   +cree\n";
        let files = split_multi_file_diff(raw);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/a.rs");
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 1);
        assert_eq!(files[1].path, "b.txt");
        assert_eq!(files[1].additions, 1);
        assert_eq!(files[1].deletions, 0);
    }
}
