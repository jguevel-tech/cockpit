//! Reconnaitre un agent IA (claude, codex, gemini...) qui tourne dans un terminal.
//!
//! C'est le repere de la barre laterale, et c'est demande toutes les 5 s pour TOUS les
//! terminaux : la question doit rester bon marche. Rien ici ne connait tmux — extrait de
//! `tmux.rs` a l'etape B2 du chantier (`docs/portabilite/plan-terminaux.md`), le service maison
//! pose exactement la meme question sur le pid de son shell.
//!
//! **LA LISTE DES CLI VIENT DU CATALOGUE DE `llm`, elle n'est pas ecrite ici.** Elle l'a ete,
//! et c'etait un piege : declarer un fournisseur ne suffisait pas a le faire reconnaitre, et
//! les deux listes divergeaient en silence. Une seule verite pour une seule chaine.

use std::collections::HashSet;
use std::sync::OnceLock;

/// Les CLI reconnus, tires du catalogue une fois pour toutes.
///
/// Memorise : cette fonction est appelee par process examine, donc des milliers de fois par
/// heure. Le catalogue est fixe pour la duree du programme, il n'y a rien a rafraichir.
fn commandes_llm() -> &'static [&'static str] {
    static LISTE: OnceLock<Vec<&'static str>> = OnceLock::new();
    LISTE.get_or_init(crate::llm::commandes_connues)
}

/// Dernier segment d'un chemin. Coupe sur `/` ET sur `\` : une ligne de commande Windows
/// s'ecrit `C:\Users\moi\claude.cmd`, et ne couper que sur `/` rendait le chemin entier,
/// donc aucune correspondance.
fn basename(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Extensions qu'un nom de programme peut porter sans changer d'identite.
///
/// `.js`/`.mjs` : un CLI lance par node. `.exe`/`.cmd`/`.bat`/`.ps1` : Windows, ou un CLI
/// installe par npm est un shim `.cmd` autour de `node.exe`. Sans ce retrait, la detection
/// ne reconnaissait RIEN sous Windows — ni `claude.cmd`, ni `node.exe` comme lanceur.
const EXTENSIONS_DE_PROGRAMME: &[&str] =
    &[".js", ".mjs", ".cjs", ".exe", ".cmd", ".bat", ".ps1"];

/// Retire l'extension d'un nom de programme, sans tenir compte de la casse : Windows ecrit
/// aussi bien `claude.cmd` que `CLAUDE.CMD`.
fn sans_extension(nom: &str) -> &str {
    for ext in EXTENSIONS_DE_PROGRAMME {
        let Some(coupe) = nom.len().checked_sub(ext.len()) else { continue };
        // `get` plutot que `split_at` : un nom finissant par un caractere multi-octets
        // n'aurait pas de frontiere a cet endroit, et `split_at` paniquerait.
        if coupe > 0 && nom.get(coupe..).is_some_and(|fin| fin.eq_ignore_ascii_case(ext)) {
            return &nom[..coupe];
        }
    }
    nom
}

pub fn est_commande_llm(cmd: &str) -> bool {
    commandes_llm().contains(&sans_extension(cmd))
}

/// Le BINAIRE REEL du process est-il un CLI LLM ?
///
/// Necessaire parce que argv[0] peut mentir : constate le 2026-08-14, un claude natif lance
/// depuis un shell ou trainait la variable APPIMAGE (fuite corrigee en 0.6.7) s'affichait
/// comme `.../target/release/cockpit -r` dans `ps` ET dans `pane_current_command` — la
/// detection par nom de commande devenait aveugle. `/proc/exe` pointe, lui, sur le vrai
/// binaire (`~/.local/share/claude/versions/2.1.231`) : on matche chaque composant du chemin
/// (le basename est un numero de version, c'est le dossier `claude` qui signe).
fn chemin_est_llm(exe: &std::path::Path) -> bool {
    exe.components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(est_commande_llm)
    })
}

/// Sous Linux, le lien `/proc/<pid>/exe` : une lecture, pas d'enumeration.
#[cfg(target_os = "linux")]
fn exe_est_llm(pid: u32) -> bool {
    std::fs::read_link(format!("/proc/{}/exe", pid))
        .map(|exe| chemin_est_llm(&exe))
        .unwrap_or(false)
}

/// Une ligne de commande complete correspond-elle a un CLI LLM ?
/// Reconnait `claude ...`, `/usr/bin/claude`, mais aussi `node /path/gemini.js`.
pub fn arguments_sont_llm(args: &str) -> bool {
    let mut tokens = args.split_whitespace();
    let Some(first) = tokens.next() else { return false };
    let base = sans_extension(basename(first));
    if est_commande_llm(base) {
        return true;
    }
    if matches!(base, "node" | "bun" | "deno" | "python" | "python3") {
        if let Some(second) = tokens.next() {
            return est_commande_llm(basename(second));
        }
    }
    false
}

/// Vue des process, juste ce qu'il faut pour reconnaitre un CLI LLM sous un shell.
///
/// NE PAS revenir a une enumeration globale (`ps -e`, `sysinfo::refresh_processes`) : cette
/// detection tourne toutes les 5 s pour la sidebar, et les deux lisent les ~1000 process de
/// la machine pour en regarder une poignee. Mesure du 2026-08-20 (1074 process, charge 0,6) :
/// `ps -e -o pid=,ppid=,args=` = 47 ms par passe, et jusqu'a 1 s sous la charge d'agents qui
/// tournent — le cas d'usage meme de Cockpit. Descendre l'arbre des seuls shells de terminaux
/// avec sortie des qu'un LLM est trouve : 0,35 ms, 9 process lus au lieu de 1074.
#[cfg(target_os = "linux")]
pub struct ArbreProcess;

#[cfg(target_os = "linux")]
impl ArbreProcess {
    pub fn nouveau() -> Self {
        Self
    }

    /// Enfants directs, via `/proc/<pid>/task/<tid>/children`.
    ///
    /// On parcourt TOUTES les taches et pas seulement le thread principal : un enfant est
    /// rattache au thread qui l'a cree, et un `node` (donc claude, codex...) fork depuis un
    /// thread de travail. Ne garder que `task/<pid>` rendait ces descendants invisibles.
    fn enfants(&self, pid: u32) -> Vec<u32> {
        let Ok(taches) = std::fs::read_dir(format!("/proc/{}/task", pid)) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for tache in taches.flatten() {
            // Un process peut mourir en cours de lecture : son absence n'est pas une panne.
            if let Ok(liste) = std::fs::read_to_string(tache.path().join("children")) {
                out.extend(liste.split_whitespace().filter_map(|p| p.parse::<u32>().ok()));
            }
        }
        out
    }

    fn est_llm(&self, pid: u32) -> bool {
        // /proc/<pid>/cmdline : arguments separes par des NUL. Meme process disparu = pas
        // une panne, on repond simplement « pas un LLM ».
        let ligne = std::fs::read(format!("/proc/{}/cmdline", pid))
            .map(|brut| String::from_utf8_lossy(&brut).replace('\0', " "))
            .unwrap_or_default();
        arguments_sont_llm(ligne.trim()) || exe_est_llm(pid)
    }
}

/// Sans `/proc` (macOS), il n'y a pas de moyen de descendre l'arbre a la demande : on
/// construit la table complete une fois par passe, avec sysinfo (deja une dependance)
/// plutot qu'en analysant la sortie texte de `ps`.
#[cfg(not(target_os = "linux"))]
pub struct ArbreProcess {
    enfants: std::collections::HashMap<u32, Vec<u32>>,
    ligne: std::collections::HashMap<u32, String>,
    /// Le binaire REEL de chaque process. `Process::exe()` de sysinfo passe par
    /// `proc_pidpath` sous macOS et `GetModuleFileNameExW` sous Windows : la garantie
    /// anti-usurpation d'argv survit sur les trois systemes. La lecture directe de
    /// `/proc/<pid>/exe` rendait, elle, toujours faux ailleurs que sous Linux — la moitie
    /// de la detection etait donc morte sans que rien ne le dise.
    exe: std::collections::HashMap<u32, std::path::PathBuf>,
}

#[cfg(not(target_os = "linux"))]
impl ArbreProcess {
    pub fn nouveau() -> Self {
        use std::collections::HashMap;
        use sysinfo::{ProcessRefreshKind, RefreshKind, System};
        let sys = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::new()),
        );
        let mut enfants: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut ligne: HashMap<u32, String> = HashMap::new();
        let mut exe: HashMap<u32, std::path::PathBuf> = HashMap::new();
        for (pid, process) in sys.processes() {
            let pid = pid.as_u32();
            if let Some(parent) = process.parent() {
                enfants.entry(parent.as_u32()).or_default().push(pid);
            }
            ligne.insert(pid, process.cmd().join(" "));
            if let Some(chemin) = process.exe() {
                exe.insert(pid, chemin.to_path_buf());
            }
        }
        Self { enfants, ligne, exe }
    }

    fn enfants(&self, pid: u32) -> Vec<u32> {
        self.enfants.get(&pid).cloned().unwrap_or_default()
    }

    fn est_llm(&self, pid: u32) -> bool {
        self.ligne.get(&pid).is_some_and(|l| arguments_sont_llm(l))
            || self.exe.get(&pid).is_some_and(|e| chemin_est_llm(e))
    }
}

impl ArbreProcess {
    /// Un CLI LLM tourne-t-il quelque part sous `racine` (elle comprise) ?
    ///
    /// Sortie DES QUE trouve : c'est ce qui rend la passe gratuite sur les terminaux ou un
    /// agent tourne, justement les plus charges.
    pub fn contient_un_llm(&self, racine: u32) -> bool {
        let mut vus: HashSet<u32> = HashSet::new();
        let mut pile = vec![racine];
        while let Some(pid) = pile.pop() {
            // Un cycle est impossible dans un arbre de process, mais un pid reutilise
            // pendant le parcours suffirait a boucler : on ne visite chacun qu'une fois.
            if !vus.insert(pid) {
                continue;
            }
            if self.est_llm(pid) {
                return true;
            }
            pile.extend(self.enfants(pid));
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::{arguments_sont_llm, sans_extension};

    #[test]
    fn les_extensions_de_programme_sont_retirees() {
        assert_eq!(sans_extension("claude.cmd"), "claude");
        assert_eq!(sans_extension("CLAUDE.CMD"), "CLAUDE");
        assert_eq!(sans_extension("node.exe"), "node");
        assert_eq!(sans_extension("gemini.js"), "gemini");
        // Rien a retirer : le nom passe tel quel, sans tronquer.
        assert_eq!(sans_extension("claude"), "claude");
        assert_eq!(sans_extension(".exe"), ".exe");
        assert_eq!(sans_extension("resume.md"), "resume.md");
        // Un nom accentue ne doit pas faire paniquer la coupe.
        assert_eq!(sans_extension("resumé"), "resumé");
    }

    #[test]
    fn detects_llm_command_lines() {
        assert!(arguments_sont_llm("claude --resume abc"));
        assert!(arguments_sont_llm("/usr/local/bin/claude"));
        assert!(arguments_sont_llm("node /home/x/.npm/bin/gemini.js chat"));
        assert!(arguments_sont_llm("python3 /opt/aider serve"));
        assert!(arguments_sont_llm("codex"));
    }

    /// Une ligne de commande Windows n'a pas le meme separateur, et ses programmes portent
    /// une extension : sans la coupe sur `\` ni le retrait de `.cmd`/`.exe`, rien n'etait
    /// jamais reconnu la-bas.
    #[test]
    fn detects_llm_on_windows_paths() {
        assert!(arguments_sont_llm(r"C:\Users\moi\AppData\npm\claude.cmd --resume abc"));
        assert!(arguments_sont_llm(r"C:\tools\nodejs\node.exe C:\x\gemini.js"));
    }

    /// LIMITE CONNUE, et c'est pour ca que le controle du binaire reel compte : la ligne de
    /// commande est decoupee sur les ESPACES, donc un chemin qui en contient (`C:\Program
    /// Files\...`, tres courant sous Windows) n'est pas reconnu ici. La reconnaissance passe
    /// alors par `chemin_est_llm`, qui lit le binaire du process et non ses arguments.
    #[test]
    fn un_chemin_avec_espace_echappe_a_la_ligne_de_commande() {
        assert!(!arguments_sont_llm(r"C:\Program Files\nodejs\node.exe C:\x\gemini.js"));
    }

    #[test]
    fn ignores_normal_commands() {
        assert!(!arguments_sont_llm("zsh"));
        assert!(!arguments_sont_llm("vim notes-claude.md"));
        assert!(!arguments_sont_llm("tail -f claude.log"));
        assert!(!arguments_sont_llm("node server.js"));
        assert!(!arguments_sont_llm(""));
    }

    /// Le process du test lui-meme n'est pas un agent, et l'arbre ne doit pas boucler
    /// dessus. Verifie aussi que descendre un arbre reel ne panique pas.
    #[test]
    fn l_arbre_du_test_ne_contient_pas_d_agent() {
        let arbre = super::ArbreProcess::nouveau();
        assert!(!arbre.contient_un_llm(std::process::id()));
    }
}
