use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Disks, RefreshKind, System};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disks: Vec<DiskMetrics>,
    pub hostname: String,
    /// Duree depuis le demarrage, en SECONDES : la mise en forme (« 3j 4h 12m ») est du
    /// texte affiche, elle appartient donc aux catalogues de traduction, pas au Rust.
    pub uptime_secs: u64,
    /// Le systeme et sa version, lisibles : « Ubuntu 26.04 », « Windows 11 »,
    /// « macOS 14.6 ». Remplace `kernel_version`, qui rendait « 22631 » sous Windows et
    /// « 23.6.0 » sous macOS — deux nombres que personne ne sait lire.
    pub os_version: String,
    /// Vrai quand la seule facon d'arreter un processus est de le TUER (voir
    /// `process::ARRET_FORCE`). L'interface en tire son libelle.
    pub kill_is_forced: bool,
    pub top_cpu: Vec<super::process::ProcessMetrics>,
    pub top_memory: Vec<super::process::ProcessMetrics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub usage_percent: f64,
    pub cores: usize,
    pub model_name: String,
    pub per_core: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub percent: f64,
    pub swap_total: u64,
    pub swap_used: u64,
    /// Le decoupage fin de la memoire, quand le systeme sait le donner — c'est-a-dire sous
    /// LINUX seulement.
    ///
    /// `sysinfo` n'expose que sept nombres de memoire et AUCUNE notion de cache, de
    /// buffers ou de partage, sur aucune plateforme : ce detail est du code natif par
    /// systeme, ou rien. Et les categories ne se traduisent pas d'un systeme a l'autre —
    /// macOS compte de la memoire *compressee* que Linux n'a pas, Windows n'a ni
    /// « buffers » ni « memoire partagee ». Le socle (total, utilise, swap) est donc commun
    /// aux trois, et ce supplement reste Linux : `None` ailleurs, et l'interface masque le
    /// panneau au lieu d'afficher cinq barres a zero.
    ///
    /// `None` aussi sous Linux quand `/proc/meminfo` est illisible : un echec de lecture ne
    /// doit pas fabriquer des zeros qui ressemblent a une mesure.
    pub detail: Option<MemoryDetail>,
}

/// Assemble les nombres de memoire tels que le systeme les donne.
///
/// ## « Utilise » se DEMANDE, il ne se calcule pas
///
/// Ce code faisait `total - disponible`. C'est juste sous Linux et sous Windows, ou c'est
/// exactement la definition qu'ils emploient. **Sous macOS, c'est faux** : les deux nombres y
/// sont calcules separement, a partir de champs differents de `vm_statistics64`, et leur somme
/// ne fait PAS le total.
///
/// - disponible = libre + inactif + purgeable - compresse
/// - utilise    = actif + verrouille + compresse + speculatif
///
/// macOS garde le « libre » tres bas par choix et compresse beaucoup. Quand la memoire
/// compressee depasse la somme des autres, le « disponible » tombe a zero — et `total - 0`
/// annonce **100 % en permanence**. C'est ce qu'un utilisateur mac a signale le 2026-08-24 :
/// une alerte de memoire pleine qui ne s'eteint jamais, alors que `top` dit le contraire.
///
/// On demande donc les deux au systeme. Aucun changement sous Linux ni sous Windows, ou
/// `used_memory()` rend precisement `total - disponible` (verifie dans la source de la crate).
///
/// Le pourcentage est BORNE a 100 : sur macOS rien ne garantit que « utilise » reste sous le
/// total, et une jauge a 103 % se lit comme un bug d'affichage.
fn assembler_la_memoire(
    total: u64,
    used: u64,
    available: u64,
    swap_total: u64,
    swap_used: u64,
    detail: Option<MemoryDetail>,
) -> MemoryMetrics {
    let percent = if total > 0 {
        (used as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    MemoryMetrics {
        total,
        used,
        available,
        percent,
        swap_total,
        swap_used,
        detail,
    }
}

/// Le detail memoire de Linux, lu dans `/proc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDetail {
    pub cached: u64,
    pub buffers: u64,
    pub shmem: u64,
    pub s_reclaimable: u64,
    pub zfs_arc: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub mount: String,
    pub device: String,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub percent: f64,
}

pub struct Collector {
    sys: System,
    disks: Disks,
    /// Rafraichie en meme temps que la liste des disques.
    lecture_seule: std::collections::HashSet<String>,
    disk_refresh_counter: u8,
    /// Static system info cached once
    hostname: String,
    os_version: String,
    cpu_model: String,
    cpu_cores: usize,
}

/// Systemes de fichiers qui sont des images montees, quel que soit le systeme.
///
/// Complement du critere principal (voir `montages_en_lecture_seule`) : ces types-la sont en
/// lecture seule par nature, et cette liste vaut aussi la ou `/proc/mounts` n'existe pas.
const IMAGES_MONTEES: [&str; 4] = ["squashfs", "iso9660", "erofs", "cramfs"];

fn est_une_image_montee(systeme_de_fichiers: &std::ffi::OsStr) -> bool {
    systeme_de_fichiers
        .to_str()
        .is_some_and(|nom| IMAGES_MONTEES.contains(&nom.to_ascii_lowercase().as_str()))
}

/// Les points de montage montes en LECTURE SEULE. Vide la ou `/proc/mounts` n'existe pas.
///
/// C'est LE critere qui decide si une entree est un disque : peut-on y liberer de la place ?
/// Sur un montage en lecture seule, non — donc « disque presque plein » n'a aucun sens, et
/// l'entree n'a rien a faire dans le monitoring non plus.
///
/// Le cas qui l'a impose est le NOTRE. Une AppImage se monte elle-meme, pleine a 100 % par
/// construction, et la cloche annoncait « /tmp/.mount_cockpiXXXX : 100 % utilises » a chaque
/// lancement. Une premiere tentative a filtre le TYPE `squashfs` : ca ne marche pas, et c'est
/// une lecon a garder. Une AppImage se monte par FUSE, donc le type reel est
/// **`fuse.cockpit`** — le sous-type porte le nom du programme, pas celui du format. Mesure
/// sur la machine, ligne brute de `/proc/mounts` :
///
/// ```text
/// cockpit /tmp/.mount_cockpiOLFNpL fuse.cockpit ro,nosuid,nodev,relatime,... 0 0
/// ```
///
/// Le type n'etait donc pas devinable : il fallait lire. L'option `ro`, elle, est explicite.
///
/// `sysinfo` 0.30 n'expose pas cette information (`is_read_only` n'existe qu'a partir de
/// 0.31), d'ou la lecture directe. Les points de montage y sont echappes a la mode fstab
/// (`\040` pour une espace) : il faut les desechapper pour les comparer a ce que rend
/// `sysinfo`.
fn montages_en_lecture_seule() -> std::collections::HashSet<String> {
    #[cfg(target_os = "linux")]
    {
        let Ok(brut) = std::fs::read_to_string("/proc/mounts") else {
            return std::collections::HashSet::new();
        };
        lire_montages_en_lecture_seule(&brut)
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::collections::HashSet::new()
    }
}

/// La partie PURE de la lecture ci-dessus, pour qu'elle soit testable sans machine.
///
/// `#[cfg]` comme son appelant : ailleurs elle serait du code mort, et le projet exige zero
/// avertissement.
#[cfg(target_os = "linux")]
fn lire_montages_en_lecture_seule(brut: &str) -> std::collections::HashSet<String> {
    brut.lines()
        .filter_map(|ligne| {
            let mut champs = ligne.split(' ');
            let _peripherique = champs.next()?;
            let point = champs.next()?;
            let _type = champs.next()?;
            let options = champs.next()?;
            // `ro` est une option a part entiere, pas un prefixe : `rootcontext=...` ne doit
            // pas compter, et `relatime` non plus.
            options
                .split(',')
                .any(|o| o == "ro")
                .then(|| desechapper_montage(point))
        })
        .collect()
}

/// Desechappe un point de montage de `/proc/mounts` (echappement octal a la mode fstab).
#[cfg(target_os = "linux")]
fn desechapper_montage(brut: &str) -> String {
    let mut sortie = String::with_capacity(brut.len());
    let octets = brut.as_bytes();
    let mut i = 0;
    while i < octets.len() {
        if octets[i] == b'\\' && i + 3 < octets.len() {
            if let Some(valeur) = std::str::from_utf8(&octets[i + 1..i + 4])
                .ok()
                .and_then(|chiffres| u8::from_str_radix(chiffres, 8).ok())
            {
                sortie.push(valeur as char);
                i += 4;
                continue;
            }
        }
        sortie.push(octets[i] as char);
        i += 1;
    }
    sortie
}

impl Collector {
    pub fn new() -> Self {
        // Minimal init: only CPU info (for model/cores), no processes
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_cpu(CpuRefreshKind::everything()),
        );
        sys.refresh_cpu_usage();

        let cpu_model = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();
        let cpu_cores = sys.cpus().len();

        Self {
            sys,
            disks: Disks::new_with_refreshed_list(),
            lecture_seule: montages_en_lecture_seule(),
            disk_refresh_counter: 0,
            hostname: System::host_name().unwrap_or_default(),
            os_version: System::long_os_version().unwrap_or_default(),
            cpu_model,
            cpu_cores,
        }
    }

    pub fn collect(&mut self) -> SystemMetrics {
        // Targeted refreshes instead of refresh_all()
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes();

        let cpu = self.collect_cpu();
        let memory = self.collect_memory();
        let disks = self.collect_disks();
        let (top_cpu, top_memory) = super::process::collect_processes(&self.sys);

        SystemMetrics {
            cpu,
            memory,
            disks,
            hostname: self.hostname.clone(),
            uptime_secs: System::uptime(),
            os_version: self.os_version.clone(),
            kill_is_forced: super::process::ARRET_FORCE,
            top_cpu,
            top_memory,
        }
    }

    fn collect_cpu(&self) -> CpuMetrics {
        let per_core: Vec<f64> = self.sys.cpus().iter().map(|c| c.cpu_usage() as f64).collect();
        let global = if per_core.is_empty() {
            0.0
        } else {
            per_core.iter().sum::<f64>() / per_core.len() as f64
        };

        CpuMetrics {
            usage_percent: global,
            cores: self.cpu_cores,
            model_name: self.cpu_model.clone(),
            per_core,
        }
    }

    fn collect_memory(&self) -> MemoryMetrics {
        assembler_la_memoire(
            self.sys.total_memory(),
            self.sys.used_memory(),
            self.sys.available_memory(),
            self.sys.total_swap(),
            self.sys.used_swap(),
            lire_detail_memoire(),
        )
    }

    fn collect_disks(&mut self) -> Vec<DiskMetrics> {
        // Refresh disk list only every ~10 collects (~30s in live mode)
        self.disk_refresh_counter += 1;
        if self.disk_refresh_counter >= 10 {
            self.disk_refresh_counter = 0;
            self.disks = Disks::new_with_refreshed_list();
            self.lecture_seule = montages_en_lecture_seule();
        }

        // PAS de filtre maison sur les points de montage. Les six chemins Unix qui etaient
        // ecrits ici ne matchaient RIEN sous Windows (`C:\`) et laissaient tomber
        // `/System/Volumes/Data` sous macOS, c'est-a-dire le volume ou sont les fichiers de
        // l'utilisateur : une liste de disques vide, sans message. `sysinfo` filtre deja, et
        // mieux, sur les trois systemes — il ecarte les pseudo-systemes de fichiers et les
        // montages snap sous Linux, les instantanes APFS et les volumes reseau sous macOS,
        // et ne garde que `DRIVE_FIXED`/`DRIVE_REMOVABLE` sous Windows. Effet de bord voulu
        // sous Linux : un disque monte sur `/mnt/data` ou `/srv` apparait enfin.
        self.disks
            .iter()
            .filter(|d| {
                // Un montage ou l'on ne peut RIEN liberer n'est pas un disque : ni dans la
                // liste, ni dans l'alerte. Le cas d'ecole est notre propre AppImage.
                !est_une_image_montee(d.file_system())
                    && !self.lecture_seule.contains(&*d.mount_point().to_string_lossy())
            })
            .map(|d| {
                let total = d.total_space();
                let free = d.available_space();
                let used = total.saturating_sub(free);
                let percent = if total > 0 {
                    used as f64 / total as f64 * 100.0
                } else {
                    0.0
                };
                DiskMetrics {
                    mount: d.mount_point().to_string_lossy().to_string(),
                    // Sous Windows, `name()` est l'etiquette du volume et peut etre VIDE :
                    // l'affichage donnait « C:\ () ». Le frontend n'affiche rien plutot
                    // qu'une parenthese vide, il faut donc que le champ soit vraiment vide.
                    device: d.name().to_string_lossy().trim().to_string(),
                    total,
                    used,
                    free,
                    percent,
                }
            })
            .collect()
    }

    /// Access the inner System for kill_process reuse
    pub fn system(&self) -> &System {
        &self.sys
    }
}

/// Le detail memoire, Linux seulement (voir `MemoryMetrics::detail`).
#[cfg(target_os = "linux")]
fn lire_detail_memoire() -> Option<MemoryDetail> {
    let brut = std::fs::read_to_string("/proc/meminfo").ok()?;
    let (cached, buffers, shmem, s_reclaimable) = parse_proc_meminfo(&brut);
    Some(MemoryDetail {
        cached,
        buffers,
        shmem,
        s_reclaimable,
        zfs_arc: read_zfs_arc_size(),
    })
}

#[cfg(not(target_os = "linux"))]
fn lire_detail_memoire() -> Option<MemoryDetail> {
    None
}

/// Analyse separee de la lecture du fichier : c'est ce qui la rend testable.
#[cfg(target_os = "linux")]
fn parse_proc_meminfo(content: &str) -> (u64, u64, u64, u64) {
    let mut cached: u64 = 0;
    let mut buffers: u64 = 0;
    let mut shmem: u64 = 0;
    let mut s_reclaimable: u64 = 0;

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let val_kb: u64 = parts[1].parse().unwrap_or(0);
        let val_bytes = val_kb * 1024;
        match parts[0] {
            "Cached:" => cached = val_bytes,
            "Buffers:" => buffers = val_bytes,
            "Shmem:" => shmem = val_bytes,
            "SReclaimable:" => s_reclaimable = val_bytes,
            _ => {}
        }
    }

    (cached, buffers, shmem, s_reclaimable)
}

#[cfg(target_os = "linux")]
fn read_zfs_arc_size() -> u64 {
    let content = match std::fs::read_to_string("/proc/spl/kstat/zfs/arcstats") {
        Ok(c) => c,
        Err(_) => return 0,
    };

    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 && parts[0] == "size" {
            return parts[2].parse().unwrap_or(0);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LE CAS macOS, reproduit avec ses vrais nombres.
    ///
    /// Sur un Mac de 16 Gio bien rempli, `sysinfo` rend « utilise » et « disponible »
    /// separement, et leur somme ne fait pas le total. L'ancien calcul `total - disponible`
    /// annoncait 100 % alors que le systeme dit 56 %. Cet essai tombe si l'on y revient.
    #[test]
    fn le_pourcentage_suit_ce_que_le_systeme_dit_utilise() {
        let gio = 1024 * 1024 * 1024;
        // Memoire compressee superieure a libre + inactif + purgeable : « disponible » tombe a
        // zero, ce qui est la situation ordinaire d'un Mac allume depuis quelques jours.
        let mesures = assembler_la_memoire(16 * gio, 9 * gio, 0, 0, 0, None);

        assert_eq!(
            mesures.used,
            9 * gio,
            "« utilise » vient du systeme, pas d'une soustraction",
        );
        assert!(
            (mesures.percent - 56.25).abs() < 0.01,
            "attendu ~56 %, obtenu {} % — le calcul est reparti de « disponible »",
            mesures.percent,
        );
        assert!(
            mesures.percent < 92.0,
            "a ce niveau l'alerte de memoire pleine se declenche et ne s'eteint jamais",
        );
    }

    /// Sous Linux et sous Windows, `used_memory()` EST `total - disponible`. Le changement ne
    /// doit donc rien y modifier : meme entree, meme resultat qu'avant.
    #[test]
    fn sous_linux_le_resultat_est_inchange() {
        let gio = 1024 * 1024 * 1024;
        let total = 32 * gio;
        let disponible = 20 * gio;
        let mesures = assembler_la_memoire(total, total - disponible, disponible, 0, 0, None);

        assert_eq!(mesures.used, 12 * gio);
        assert!((mesures.percent - 37.5).abs() < 0.01);
    }

    /// Rien ne garantit sous macOS que « utilise » reste sous le total : les deux nombres sont
    /// independants. Une jauge a 103 % se lit comme un bug d'affichage.
    #[test]
    fn le_pourcentage_ne_depasse_jamais_cent() {
        let gio = 1024 * 1024 * 1024;
        let mesures = assembler_la_memoire(8 * gio, 9 * gio, 0, 0, 0, None);

        assert_eq!(mesures.percent, 100.0);
    }

    /// Une machine qui ne rend pas de total ne doit pas provoquer une division par zero, ni un
    /// `NaN` qui traverserait jusqu'a la jauge.
    #[test]
    fn un_total_a_zero_ne_donne_pas_nan() {
        let mesures = assembler_la_memoire(0, 0, 0, 0, 0, None);

        assert_eq!(mesures.percent, 0.0);
    }

    /// De bout en bout, sur la machine qui fait tourner l'essai : AUCUN disque rendu ne doit
    /// etre un montage en lecture seule. Quand une AppImage tourne, cet essai voit le vrai
    /// `/tmp/.mount_*` et echouerait si le filtre etait mal branche — ce que les essais sur
    /// texte ne peuvent pas dire.
    #[test]
    #[cfg(target_os = "linux")]
    fn aucun_disque_rendu_n_est_en_lecture_seule() {
        let mut collecteur = super::Collector::new();
        let mesures = collecteur.collect();
        let seules = super::montages_en_lecture_seule();
        for disque in &mesures.disks {
            assert!(
                !seules.contains(&disque.mount),
                "{} est monte en lecture seule et ne devrait pas etre rendu comme un disque",
                disque.mount
            );
        }
    }

    /// La ligne EXACTE relevee sur la machine du mainteneur le 2026-08-21. Le type est
    /// `fuse.cockpit` et non `squashfs` : c'est ce qui a fait echouer la premiere tentative
    /// de correctif, qui filtrait le type. L'option `ro`, elle, est la.
    #[cfg(target_os = "linux")]
    #[test]
    fn le_montage_d_une_appimage_est_vu_en_lecture_seule() {
        let brut = "\
cockpit /tmp/.mount_cockpiOLFNpL fuse.cockpit ro,nosuid,nodev,relatime,user_id=1000,group_id=1000 0 0
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
/dev/nvme0n1p1 /boot/efi vfat rw,relatime,fmask=0077 0 0
/dev/sr0 /media/cdrom iso9660 ro,nosuid,nodev,relatime,uid=1000 0 0
tmpfs /dev/shm tmpfs rw,nosuid,nodev 0 0
";
        let seules = lire_montages_en_lecture_seule(brut);
        assert!(seules.contains("/tmp/.mount_cockpiOLFNpL"), "{seules:?}");
        assert!(seules.contains("/media/cdrom"), "{seules:?}");
        // Les disques ou l'on peut ecrire ne doivent PAS y etre : sinon on les ferait
        // disparaitre du monitoring, ce qui est le bug d'avant la 0.38 a l'envers.
        assert!(!seules.contains("/"), "{seules:?}");
        assert!(!seules.contains("/boot/efi"), "{seules:?}");
        assert_eq!(seules.len(), 2, "{seules:?}");
    }

    /// `rootcontext=...` commence par « ro » sans etre l'option `ro`, et `relatime` aussi.
    #[cfg(target_os = "linux")]
    #[test]
    fn ro_est_une_option_entiere_pas_un_prefixe() {
        let brut = "x /donnees ext4 rw,relatime,rootcontext=system_u:object_r:t 0 0\n";
        assert!(lire_montages_en_lecture_seule(brut).is_empty());
    }

    /// Un point de montage avec une espace est echappe en `\040` dans `/proc/mounts`, alors
    /// que `sysinfo` rend l'espace : sans desechappement, la comparaison ne trouve rien.
    #[cfg(target_os = "linux")]
    #[test]
    fn un_point_de_montage_avec_une_espace_est_desechappe() {
        let brut = "x /media/mon\\040disque iso9660 ro 0 0\n";
        let seules = lire_montages_en_lecture_seule(brut);
        assert!(seules.contains("/media/mon disque"), "{seules:?}");
    }

    /// Garde de repli, utile la ou `/proc/mounts` n'existe pas.
    #[test]
    fn une_image_montee_n_est_pas_un_disque() {
        for nom in ["squashfs", "iso9660", "erofs", "cramfs", "SquashFS"] {
            assert!(est_une_image_montee(std::ffi::OsStr::new(nom)), "{nom}");
        }
    }

    /// Ceux-la doivent passer : c'est sur eux que « disque presque plein » a un sens. `ntfs`
    /// et `apfs` comptent autant qu'`ext4` — le filtre d'avant la 0.38 etait une liste de
    /// chemins Unix, et il vidait la liste ailleurs que sous Linux.
    #[test]
    fn un_vrai_disque_reste_un_disque() {
        for nom in ["ext4", "btrfs", "xfs", "zfs", "ntfs", "apfs", "exfat", "vfat"] {
            assert!(!est_une_image_montee(std::ffi::OsStr::new(nom)), "{nom}");
        }
    }

    /// Les quatre lignes attendues sont lues, et les kilo-octets convertis en octets.
    #[test]
    #[cfg(target_os = "linux")]
    fn le_detail_memoire_se_lit_en_octets() {
        let brut = "MemTotal:       32000000 kB\n\
                    Buffers:            2048 kB\n\
                    Cached:          1024000 kB\n\
                    SReclaimable:     512000 kB\n\
                    Shmem:             65536 kB\n";
        let (cached, buffers, shmem, s_reclaimable) = parse_proc_meminfo(brut);
        assert_eq!(cached, 1_024_000 * 1024);
        assert_eq!(buffers, 2_048 * 1024);
        assert_eq!(shmem, 65_536 * 1024);
        assert_eq!(s_reclaimable, 512_000 * 1024);
    }

    /// Le detail n'existe que sous Linux : ailleurs l'interface masque le panneau, elle ne
    /// doit surtout pas recevoir des zeros qui passeraient pour une mesure.
    #[test]
    fn le_detail_memoire_est_reserve_a_linux() {
        assert_eq!(lire_detail_memoire().is_some(), cfg!(target_os = "linux"));
    }
}
