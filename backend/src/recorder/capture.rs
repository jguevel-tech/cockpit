//! Capture audio : micro + son qui sort des enceintes, chacun dans sa piste.
//!
//! LA CAPTURE EST DANS NOTRE PROCESSUS (`cpal`), plus aucun programme externe. Avant
//! aout 2026, deux binaires etaient lances et leur stdout redirige vers le fichier :
//! `pw-record` puis `parecord` en repli. Ce modele decrivait surtout les caprices de
//! version d'outils tiers (lire `--help` pour savoir si `-P` existe, interpreter deux
//! stderr) et il ne menait nulle part hors de Linux.
//!
//! **CE QUI SORT D'ICI NE CHANGE PAS** : `mic.raw` et `system.raw`, PCM s16le mono
//! 16 kHz, a l'octet pres (`pcm.rs` fait la mise au format). C'est le contrat de tout
//! l'aval — chunks de 10 min, detection de silence, en-tete WAV, fusion Moi/Eux — et
//! c'est ce qui garde ce choix reversible : une autre capture derriere la meme frontiere
//! ne se verrait pas plus loin.
//!
//! UN SEUL IDIOME, TROIS MECANISMES SYSTEME : on construit un flux d'ENTREE, et pour le
//! son systeme on le construit sur un appareil de SORTIE.
//!   - **Linux** : host PulseAudio de cpal (protocole PulseAudio reimplemente en Rust,
//!     aucune bibliotheque C de plus), et le son systeme est la source
//!     `<sink>.monitor` — le host refuse un flux d'entree sur un `Device::Sink`.
//!     Il couvre les machines PipeWire (via `pipewire-pulse`) comme les machines
//!     PulseAudio, donc aussi l'Ubuntu 22.04 qui forcait le repli `parecord`.
//!   - **Windows** : WASAPI pose `AUDCLNT_STREAMFLAGS_LOOPBACK` tout seul quand on
//!     demande une entree sur un appareil de sortie. Rien a installer.
//!   - **macOS** : Core Audio branche un « process tap » (14.4+). ATTENTION : sans
//!     identite de signature Apple, l'autorisation TCC n'est meme pas demandee et la
//!     piste ne contient QUE DES ZEROS, sans aucune erreur. C'est pour ca que le bilan
//!     distingue « piste entierement muette » de « piste avec des passages muets ».
//!
//! LE REPLI RESTE UN CONSTAT, PISTE PAR PISTE : on essaie un appareil, on regarde s'il
//! livre des octets, et on passe au suivant sinon. Jamais un diagnostic devine.

use super::pcm::Convertisseur;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Data, Device, Sample, SampleFormat, SupportedStreamConfig};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Delai laisse a un appareil pour livrer son premier lot avant de le juger mort et de
/// passer au candidat suivant. Un rappel audio arrive en general en moins de 50 ms.
const DELAI_PREMIER_LOT: Duration = Duration::from_secs(1);

/// Plafond d'attente au demarrage. `start_capture` repart des que les DEUX pistes ont
/// repondu : dans le cas normal c'est bien plus rapide (l'ancien code attendait 300 ms
/// en aveugle, qu'il y ait quelque chose a attendre ou non).
const DELAI_DEMARRAGE: Duration = Duration::from_secs(3);

/// Au-dela de ce nombre d'echantillons en attente d'ecriture, le rappel audio jette son
/// lot au lieu de laisser la memoire grossir sans fin. 30 s de 48 kHz stereo : jamais
/// atteint en pratique (l'ecriture est de ~32 ko/s), mais un plafond qui n'existe pas
/// est un plafond qu'on decouvre en manquant de memoire.
const ECHANTILLONS_EN_ATTENTE_MAX: u64 = 48_000 * 2 * 30;

const EN_ATTENTE: u8 = 0;
const ENREGISTRE: u8 = 1;
const ECHEC: u8 = 2;

/// Ce qu'on veut capter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Micro,
    Systeme,
}

impl Role {
    /// CODE, pas une phrase : c'est l'interface qui l'affiche, dans la langue choisie.
    pub fn code(self) -> &'static str {
        match self {
            Role::Micro => "mic",
            Role::Systeme => "system",
        }
    }

    fn fichier(self) -> &'static str {
        match self {
            Role::Micro => "mic.raw",
            Role::Systeme => "system.raw",
        }
    }

    /// Nom lisible dans un message d'erreur.
    fn etiquette(self) -> &'static str {
        match self {
            Role::Micro => "micro",
            Role::Systeme => "son systeme",
        }
    }
}

/// Ce que cpal sait dire de l'appareil retenu. Remplace la version de `pw-record` et le
/// serveur audio devine par `pactl` : ces deux-la n'existent plus, et ceci est portable.
#[derive(Clone, Debug)]
pub struct Fiche {
    pub hote: String,
    pub appareil: String,
    pub taux: u32,
    pub canaux: u16,
    pub format: String,
}

impl std::fmt::Display for Fiche {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} sur {} ({} Hz, {} canaux, {})",
            self.appareil, self.hote, self.taux, self.canaux, self.format
        )
    }
}

/// Ce que le thread d'une piste publie pendant l'enregistrement. Rien ici n'est lu par
/// le rappel audio : il ne fait qu'ecrire des compteurs.
struct EtatPiste {
    phase: AtomicU8,
    octets: AtomicU64,
    /// Amplitude maximale rencontree (0..32767). C'est elle qui distingue « rien recu »
    /// de « du son » : une piste a zero strict n'a jamais recu un seul octet utile.
    crete: AtomicU32,
    /// Echantillons jetes parce que l'ecriture etait en retard.
    jetes: AtomicU64,
    erreur: Mutex<Option<String>>,
    fiche: Mutex<Option<Fiche>>,
}

impl EtatPiste {
    fn neuf() -> Arc<Self> {
        Arc::new(Self {
            phase: AtomicU8::new(EN_ATTENTE),
            octets: AtomicU64::new(0),
            crete: AtomicU32::new(0),
            jetes: AtomicU64::new(0),
            erreur: Mutex::new(None),
            fiche: Mutex::new(None),
        })
    }

    fn note_erreur(&self, message: String) {
        if let Ok(mut slot) = self.erreur.lock() {
            *slot = Some(message);
        }
    }

    fn erreur(&self) -> Option<String> {
        self.erreur.lock().ok().and_then(|e| e.clone())
    }
}

/// Une piste en cours : le thread qui detient le flux cpal, et ce qu'il constate.
///
/// Le flux vit dans SON thread et n'en sort pas : `cpal::Stream` n'est pas `Send` sur la
/// plupart des systemes, alors que `CaptureHandles` doit traverser l'etat partage de
/// Tauri. Le thread construit, joue, ecrit, puis relache — personne d'autre ne le touche.
struct Piste {
    role: Role,
    etat: Arc<EtatPiste>,
    arret: Arc<AtomicBool>,
    fil: Option<std::thread::JoinHandle<()>>,
}

impl Piste {
    fn demarrer(dir: &Path, role: Role) -> Self {
        let chemin = dir.join(role.fichier());
        let etat = EtatPiste::neuf();
        let arret = Arc::new(AtomicBool::new(false));
        let (e, a) = (etat.clone(), arret.clone());
        let fil = std::thread::Builder::new()
            .name(format!("capture-{}", role.code()))
            .spawn(move || tenir_piste(&chemin, role, &e, &a))
            .ok();
        if fil.is_none() {
            etat.note_erreur("impossible de lancer le thread de capture".to_string());
            etat.phase.store(ECHEC, Ordering::Release);
        }
        Self { role, etat, arret, fil }
    }

    fn a_repondu(&self) -> bool {
        self.etat.phase.load(Ordering::Acquire) != EN_ATTENTE
    }

    fn enregistre(&self) -> bool {
        self.etat.phase.load(Ordering::Acquire) == ENREGISTRE
    }

    /// Demande l'arret et attend que le thread ait vide ce qui restait a ecrire.
    fn arreter(&mut self) -> BilanPiste {
        self.arret.store(true, Ordering::Release);
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
        let octets = self.etat.octets.load(Ordering::Acquire);
        let crete = self.etat.crete.load(Ordering::Acquire);
        BilanPiste {
            role: self.role,
            octets,
            crete,
            jetes: self.etat.jetes.load(Ordering::Acquire),
            erreur: self.etat.erreur(),
            fiche: self.etat.fiche.lock().ok().and_then(|f| f.clone()),
        }
    }
}

/// Ce qu'une piste a produit, une fois l'enregistrement arrete.
#[derive(Clone, Debug)]
pub struct BilanPiste {
    pub role: Role,
    pub octets: u64,
    pub crete: u32,
    pub jetes: u64,
    pub erreur: Option<String>,
    pub fiche: Option<Fiche>,
}

impl BilanPiste {
    /// La piste n'a recu QUE des zeros, alors qu'elle a bien recu des octets.
    ///
    /// C'est le symptome d'un tap macOS sans autorisation TCC : le flux tourne, les
    /// rappels arrivent, et tout est a zero — sans la moindre erreur. Le distinguer d'un
    /// enregistrement calme est ce qui evite le message « Aucune parole detectee »,
    /// qui envoie chercher au mauvais endroit.
    pub fn muette(&self) -> bool {
        self.octets > 0 && self.crete == 0
    }

    /// Ligne de journal : ce qui aide a comprendre une panne, sans rien deviner.
    pub fn resume(&self) -> String {
        let mut morceaux = vec![format!(
            "{} : {} ko, crete {}",
            self.role.etiquette(),
            self.octets / 1024,
            self.crete
        )];
        if let Some(f) = &self.fiche {
            morceaux.push(f.to_string());
        }
        if self.jetes > 0 {
            morceaux.push(format!("{} echantillons jetes", self.jetes));
        }
        if let Some(e) = &self.erreur {
            morceaux.push(e.clone());
        }
        morceaux.join(" — ")
    }
}

/// Bilan des deux pistes a l'arret.
#[derive(Clone, Debug)]
pub struct Bilan {
    pub micro: BilanPiste,
    pub systeme: BilanPiste,
}

impl Bilan {
    /// Quelle piste n'a recu que du silence : "mic", "system", "both" ou rien.
    ///
    /// Un CODE, comme `lost_track` : c'est l'interface qui le traduit.
    pub fn muette_code(&self) -> Option<&'static str> {
        match (self.micro.muette(), self.systeme.muette()) {
            (true, true) => Some("both"),
            (true, false) => Some("mic"),
            (false, true) => Some("system"),
            (false, false) => None,
        }
    }
}

pub struct CaptureHandles {
    micro: Piste,
    systeme: Piste,
}

/// Demarre les deux captures dans `dir` (mic.raw + system.raw).
///
/// Ne rend `Err` que si le dossier n'a pas pu etre cree : c'est l'appelant qui decide
/// quoi faire de ce que les pistes ont donne (`alive_tracks`), parce qu'une seule piste
/// vivante suffit pour enregistrer utilement.
pub async fn start_capture(dir: &Path) -> Result<CaptureHandles, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("creation dossier: {}", e))?;
    let handles = CaptureHandles {
        micro: Piste::demarrer(dir, Role::Micro),
        systeme: Piste::demarrer(dir, Role::Systeme),
    };

    // On repart des que les deux pistes ont tranche — enregistre ou echec.
    let limite = Instant::now() + DELAI_DEMARRAGE;
    while Instant::now() < limite && !(handles.micro.a_repondu() && handles.systeme.a_repondu()) {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Ok(handles)
}

impl CaptureHandles {
    /// Arrete les deux captures et rend ce qu'elles ont produit.
    ///
    /// L'arret joint les threads d'ecriture (il faut que le fichier soit complet avant
    /// que la transcription le lise), donc il part sur le pool bloquant : la bloquer sur
    /// le runtime de Tauri figerait l'interface.
    pub async fn stop(mut self) -> Bilan {
        tokio::task::spawn_blocking(move || Bilan {
            micro: self.micro.arreter(),
            systeme: self.systeme.arreter(),
        })
        .await
        .unwrap_or_else(|e| {
            // Le thread d'arret ne panique pas : il ne fait que poser un drapeau et
            // joindre. Si le pool disparait, on ne peut plus rien dire des pistes.
            log::error!("arret de la capture: {e}");
            Bilan {
                micro: BilanPiste::inconnue(Role::Micro),
                systeme: BilanPiste::inconnue(Role::Systeme),
            }
        })
    }

    /// Etat de chaque piste : (micro, son systeme).
    ///
    /// Les deux separement, et non un booleen d'ensemble : une machine peut tres bien
    /// capter une piste et pas l'autre, et refuser l'enregistrement entier pour cette
    /// raison privait l'utilisateur de celle qui marchait.
    pub fn alive_tracks(&self) -> (bool, bool) {
        (self.micro.enregistre(), self.systeme.enregistre())
    }

    /// Ce que les deux captures ont constate, piste par piste.
    pub fn startup_error(&self) -> String {
        startup_error(
            &self.micro.etat.erreur().unwrap_or_default(),
            &self.systeme.etat.erreur().unwrap_or_default(),
        )
    }
}

impl BilanPiste {
    fn inconnue(role: Role) -> Self {
        Self {
            role,
            octets: 0,
            crete: 0,
            jetes: 0,
            erreur: None,
            fiche: None,
        }
    }
}

/// Borne un texte destine a un message d'interface.
///
/// La coupe est faite ICI, a la construction du message, et non a la lecture : c'est le
/// seul endroit par ou tout passe, donc le seul ou la garantie tient quelle que soit
/// l'origine du texte.
fn borne(text: &str) -> String {
    const MAX: usize = 300;
    if text.chars().count() > MAX {
        let cut: String = text.chars().take(MAX).collect();
        format!("{cut}…")
    } else {
        text.to_string()
    }
}

/// Assemble le message d'echec a partir de ce que les captures ont constate.
///
/// Sans cela le diagnostic etait invente : le message annoncait « PipeWire indisponible ? »
/// alors que la cause reelle etait jetee. Un utilisateur n'avait aucun moyen de savoir ce
/// qui bloquait.
pub fn startup_error(mic: &str, system: &str) -> String {
    let mut details: Vec<String> = Vec::new();
    if !mic.is_empty() {
        details.push(format!("micro : {}", borne(mic)));
    }
    if !system.is_empty() && system != mic {
        details.push(format!("son systeme : {}", borne(system)));
    }

    if details.is_empty() {
        "Aucune capture audio n'a pu demarrer, sans message. Verifie qu'un serveur audio \
         tourne et qu'une entree est disponible."
            .to_string()
    } else {
        format!("Aucune capture audio n'a pu demarrer — {}", details.join(" ; "))
    }
}

// --- Un thread par piste : resolution de l'appareil, flux, ecriture ---

/// Un appareil a essayer pour une piste.
struct Candidat {
    hote: String,
    appareil: Device,
}

/// L'identifiant BACKEND de l'appareil (`alsa_output.pci-....monitor` sous PulseAudio).
///
/// A ne pas confondre avec `Display`, qui rend la description lisible (« Monitor of
/// Built-in Audio ») : c'est l'identifiant qui porte la convention `.monitor`.
fn identifiant(appareil: &Device) -> Option<String> {
    appareil.id().ok().map(|i| i.id().to_string())
}

/// Nom affichable d'un appareil : sa description si elle existe, son identifiant sinon.
fn nom_appareil(appareil: &Device) -> String {
    let affiche = appareil.to_string();
    match identifiant(appareil) {
        Some(id) if id != affiche => format!("{affiche} [{id}]"),
        _ => affiche,
    }
}

/// La source `<sink>.monitor` du sink par defaut.
///
/// Le nom en `.monitor` est une convention de PulseAudio, pas une garantie d'API : d'ou
/// le repli sur n'importe quelle source en `.monitor` si l'exacte n'existe pas. Verifie
/// au banc le 2026-08-21 (`alsa_output.pci-0000_00_1f.3.analog-stereo.monitor` trouve,
/// 3 s captees, crete 0,25 sur un ton joue a 0,25).
#[cfg(target_os = "linux")]
fn monitor_du_sink(hote: &cpal::Host) -> Result<Device, String> {
    let sink = hote
        .default_output_device()
        .ok_or_else(|| "aucune sortie audio par defaut".to_string())?;
    let nom = identifiant(&sink).ok_or_else(|| "sortie par defaut sans identifiant".to_string())?;
    let attendu = format!("{nom}.monitor");

    let appareils = hote.devices().map_err(|e| format!("{:?} : {e}", e.kind()))?;
    let mut replis = Vec::new();
    for appareil in appareils {
        match identifiant(&appareil) {
            Some(id) if id == attendu => return Ok(appareil),
            Some(id) if id.ends_with(".monitor") && appareil.supports_input() => {
                replis.push(appareil)
            }
            _ => {}
        }
    }
    replis
        .into_iter()
        .next()
        .ok_or_else(|| format!("aucune source « {attendu} »"))
}

/// Les appareils a essayer, dans l'ordre, et ce qu'on a constate en les cherchant.
#[cfg(target_os = "linux")]
fn candidats(role: Role) -> (Vec<Candidat>, Vec<String>) {
    let mut liste = Vec::new();
    let mut constats = Vec::new();

    // PulseAudio d'abord. Pour le son systeme c'est le SEUL : ALSA ne sait pas capter ce
    // qui SORT. Et c'est un host en Rust pur (le protocole est reimplemente), donc rien
    // de nouveau a embarquer dans l'AppImage.
    match cpal::host_from_id(cpal::HostId::PulseAudio) {
        Ok(hote) => match role {
            Role::Micro => match hote.default_input_device() {
                Some(appareil) => liste.push(Candidat { hote: "pulseaudio".into(), appareil }),
                None => constats.push("pulseaudio : aucune entree par defaut".into()),
            },
            Role::Systeme => match monitor_du_sink(&hote) {
                Ok(appareil) => liste.push(Candidat { hote: "pulseaudio".into(), appareil }),
                Err(e) => constats.push(format!("pulseaudio : {e}")),
            },
        },
        Err(e) => constats.push(format!("pulseaudio : {e}")),
    }

    // ALSA en dernier recours, et pour le micro seulement : sans serveur audio, c'est
    // tout ce qui reste. Le son systeme, lui, n'existe pas a ce niveau.
    if role == Role::Micro {
        match cpal::host_from_id(cpal::HostId::Alsa) {
            Ok(hote) => match hote.default_input_device() {
                Some(appareil) => liste.push(Candidat { hote: "alsa".into(), appareil }),
                None => constats.push("alsa : aucune entree par defaut".into()),
            },
            Err(e) => constats.push(format!("alsa : {e}")),
        }
    }

    (liste, constats)
}

/// Les appareils a essayer, dans l'ordre, et ce qu'on a constate en les cherchant.
///
/// Windows (WASAPI) et macOS (Core Audio) n'ont qu'un host, et le son systeme se capte
/// sur l'appareil de SORTIE : c'est la meme expression que pour le micro, la
/// bibliotheque pose le drapeau de loopback ou branche le tap.
#[cfg(not(target_os = "linux"))]
fn candidats(role: Role) -> (Vec<Candidat>, Vec<String>) {
    let hote = cpal::default_host();
    let nom = format!("{:?}", hote.id()).to_lowercase();
    let appareil = match role {
        Role::Micro => hote.default_input_device(),
        Role::Systeme => hote.default_output_device(),
    };
    match appareil {
        Some(appareil) => (vec![Candidat { hote: nom, appareil }], Vec::new()),
        None => (
            Vec::new(),
            vec![format!(
                "{nom} : aucun appareil {} par defaut",
                match role {
                    Role::Micro => "d'entree",
                    Role::Systeme => "de sortie",
                }
            )],
        ),
    }
}

/// La forme sous laquelle l'appareil livre ses echantillons.
///
/// `default_input_config` d'abord ; un appareil de SORTIE utilise en loopback le refuse
/// (WASAPI : « Device does not support input »), et c'est alors la forme du melange qui
/// sort qu'il faut demander. Une seule expression pour les trois systemes.
fn config_capture(appareil: &Device) -> Result<SupportedStreamConfig, String> {
    appareil
        .default_input_config()
        .or_else(|_| appareil.default_output_config())
        .map_err(|e| format!("{:?} : {e}", e.kind()))
}

/// Convertit un lot livre par le materiel en flottants -1..1.
///
/// Le materiel choisit son format : 32 bits entiers sur la machine du banc, souvent
/// `f32` ailleurs. `Sample`/`FromSample` de cpal font la mise a l'echelle, y compris
/// pour les formats non signes (dont l'origine est au milieu de la plage).
fn en_flottants(donnees: &Data) -> Vec<f32> {
    macro_rules! convertir {
        ($t:ty) => {
            donnees
                .as_slice::<$t>()
                .map(|s| s.iter().map(|v| f32::from_sample(*v)).collect())
                .unwrap_or_default()
        };
    }
    match donnees.sample_format() {
        SampleFormat::I8 => convertir!(i8),
        SampleFormat::I16 => convertir!(i16),
        SampleFormat::I24 => convertir!(cpal::I24),
        SampleFormat::I32 => convertir!(i32),
        SampleFormat::I64 => convertir!(i64),
        SampleFormat::U8 => convertir!(u8),
        SampleFormat::U16 => convertir!(u16),
        SampleFormat::U24 => convertir!(cpal::U24),
        SampleFormat::U32 => convertir!(u32),
        SampleFormat::U64 => convertir!(u64),
        SampleFormat::F32 => convertir!(f32),
        SampleFormat::F64 => convertir!(f64),
        // Ecarte avant l'ouverture du flux par `format_gere` : le rappel audio ne peut
        // pas remonter d'erreur, il ne doit donc plus y avoir de cas inconnu ici.
        _ => Vec::new(),
    }
}

/// Les formats que `en_flottants` sait convertir. Un format inconnu (DSD, ou un ajout
/// futur de cpal) est refuse a l'OUVERTURE, la ou l'erreur peut encore etre dite.
fn format_gere(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::I8
            | SampleFormat::I16
            | SampleFormat::I24
            | SampleFormat::I32
            | SampleFormat::I64
            | SampleFormat::U8
            | SampleFormat::U16
            | SampleFormat::U24
            | SampleFormat::U32
            | SampleFormat::U64
            | SampleFormat::F32
            | SampleFormat::F64
    )
}

/// Un flux ouvert, avec de quoi le vider.
struct Capture {
    flux: cpal::Stream,
    lots: Receiver<Vec<f32>>,
    convertisseur: Convertisseur,
    en_attente: Arc<AtomicU64>,
}

fn ouvrir(candidat: &Candidat, etat: &Arc<EtatPiste>) -> Result<Capture, String> {
    let config = config_capture(&candidat.appareil)?;
    let format = config.sample_format();
    if !format_gere(format) {
        return Err(format!("format d'echantillon non gere : {format:?}"));
    }
    if config.channels() == 0 {
        return Err("appareil sans canal d'entree".to_string());
    }

    let fiche = Fiche {
        hote: candidat.hote.clone(),
        appareil: nom_appareil(&candidat.appareil),
        taux: config.sample_rate(),
        canaux: config.channels(),
        format: format!("{format:?}"),
    };

    let (envoi, lots) = std::sync::mpsc::channel::<Vec<f32>>();
    let en_attente = Arc::new(AtomicU64::new(0));
    let compteur = en_attente.clone();
    let etat_donnees = etat.clone();
    let etat_erreur = etat.clone();

    let flux = candidat
        .appareil
        .build_input_stream_raw(
            config.config(),
            format,
            move |donnees: &Data, _: &cpal::InputCallbackInfo| {
                // UN RAPPEL AUDIO NE DOIT RIEN FAIRE DE LONG. Ici : une conversion en
                // flottants et un depot dans la file. Le melange des canaux, le
                // reechantillonnage et l'ecriture disque sont le travail du thread de la
                // piste, de l'autre cote de la file.
                if compteur.load(Ordering::Relaxed) > ECHANTILLONS_EN_ATTENTE_MAX {
                    etat_donnees
                        .jetes
                        .fetch_add(donnees.len() as u64, Ordering::Relaxed);
                    return;
                }
                let lot = en_flottants(donnees);
                compteur.fetch_add(lot.len() as u64, Ordering::Relaxed);
                // Le seul echec possible est un recepteur ferme, c'est-a-dire un thread
                // de piste deja termine : il n'y a plus personne a qui le dire.
                let _ = envoi.send(lot);
            },
            move |e: cpal::Error| {
                etat_erreur.note_erreur(format!("{:?} : {e}", e.kind()));
            },
            None,
        )
        .map_err(|e| format!("{:?} : {e}", e.kind()))?;

    flux.play().map_err(|e| format!("demarrage : {e}"))?;

    if let Ok(mut slot) = etat.fiche.lock() {
        *slot = Some(fiche);
    }
    Ok(Capture {
        flux,
        lots,
        convertisseur: Convertisseur::new(config.sample_rate(), config.channels()),
        en_attente,
    })
}

/// Le thread d'une piste : essaie les appareils, puis ecrit jusqu'a l'arret.
fn tenir_piste(chemin: &Path, role: Role, etat: &Arc<EtatPiste>, arret: &AtomicBool) {
    // `append` : une piste ne doit jamais effacer ce qui a deja ete capture.
    let fichier = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(chemin);
    let mut fichier = match fichier {
        Ok(f) => f,
        Err(e) => {
            etat.note_erreur(format!("creation {} : {e}", chemin.display()));
            etat.phase.store(ECHEC, Ordering::Release);
            return;
        }
    };

    let (candidats, mut constats) = candidats(role);
    for candidat in candidats {
        match ouvrir(&candidat, etat) {
            Ok(capture) => {
                match enregistrer(capture, &mut fichier, etat, arret) {
                    Ok(()) => return,
                    // Le flux s'est ouvert mais n'a rien livre : on le relache et on
                    // essaie l'appareil suivant. C'est le meme principe qu'avant — on
                    // constate, on ne devine pas.
                    Err(e) => constats.push(format!("{} : {e}", candidat.hote)),
                }
            }
            Err(e) => constats.push(format!("{} : {e}", candidat.hote)),
        }
        if arret.load(Ordering::Acquire) {
            break;
        }
    }

    if let Some(deja) = etat.erreur() {
        constats.push(deja);
    }
    etat.note_erreur(if constats.is_empty() {
        format!("aucun appareil pour le {}", role.etiquette())
    } else {
        constats.join(" / ")
    });
    etat.phase.store(ECHEC, Ordering::Release);
}

/// Vide la file dans le fichier jusqu'a l'arret.
///
/// Rend `Err` quand le flux n'a livre AUCUN lot dans le delai : rien n'a alors ete
/// ecrit, l'appelant peut essayer un autre appareil.
fn enregistrer(
    capture: Capture,
    fichier: &mut std::fs::File,
    etat: &Arc<EtatPiste>,
    arret: &AtomicBool,
) -> Result<(), String> {
    let Capture { flux, lots, mut convertisseur, en_attente } = capture;
    let limite_premier_lot = Instant::now() + DELAI_PREMIER_LOT;

    let mut ecrire = |octets: Vec<u8>, etat: &Arc<EtatPiste>| {
        if octets.is_empty() {
            return;
        }
        let crete = super::wav::max_amplitude(&octets) as u32;
        etat.crete.fetch_max(crete, Ordering::Relaxed);
        match fichier.write_all(&octets) {
            Ok(()) => {
                etat.octets.fetch_add(octets.len() as u64, Ordering::Relaxed);
                // La piste n'est declaree enregistrante que quand des octets sont DANS le
                // fichier : c'est la seule preuve que l'appareil livre vraiment.
                let _ = etat.phase.compare_exchange(
                    EN_ATTENTE,
                    ENREGISTRE,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                );
            }
            Err(e) => etat.note_erreur(format!("ecriture : {e}")),
        }
    };

    while !arret.load(Ordering::Acquire) {
        match lots.recv_timeout(Duration::from_millis(50)) {
            Ok(lot) => {
                en_attente.fetch_sub(lot.len() as u64, Ordering::Relaxed);
                ecrire(convertisseur.pousser(&lot), etat);
            }
            Err(RecvTimeoutError::Timeout) => {
                if etat.phase.load(Ordering::Acquire) == EN_ATTENTE
                    && Instant::now() > limite_premier_lot
                {
                    let motif = etat
                        .erreur()
                        .unwrap_or_else(|| "aucun octet recu".to_string());
                    return Err(motif);
                }
            }
            // L'emetteur est mort avec le flux : il n'y a plus rien a attendre.
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    // Le flux d'abord : plus aucun rappel ne depose derriere nous.
    drop(flux);
    while let Ok(lot) = lots.try_recv() {
        ecrire(convertisseur.pousser(&lot), etat);
    }
    ecrire(convertisseur.fin(), etat);
    if let Err(e) = fichier.flush() {
        etat.note_erreur(format!("ecriture : {e}"));
    }
    if etat.phase.load(Ordering::Acquire) == EN_ATTENTE {
        // L'arret est arrive avant le premier octet : la piste n'a rien enregistre, et
        // elle doit le dire au lieu de rester indecise.
        etat.note_erreur("arrete avant le premier octet".to_string());
        etat.phase.store(ECHEC, Ordering::Release);
    }
    Ok(())
}

/// Ce que la capture sait dire de la machine, sans enregistrer.
///
/// Remplace `pw_record_version()` et `audio_server_from_pactl()` de la fiche de
/// diagnostic : le host retenu et les deux appareils, avec leur format natif.
pub fn fiche_audio() -> String {
    [Role::Micro, Role::Systeme]
        .iter()
        .map(|role| {
            let (candidats, constats) = candidats(*role);
            let texte = match candidats.first() {
                Some(c) => match config_capture(&c.appareil) {
                    Ok(config) => Fiche {
                        hote: c.hote.clone(),
                        appareil: nom_appareil(&c.appareil),
                        taux: config.sample_rate(),
                        canaux: config.channels(),
                        format: format!("{:?}", config.sample_format()),
                    }
                    .to_string(),
                    Err(e) => e,
                },
                None if constats.is_empty() => "aucun appareil".to_string(),
                None => constats.join(" / "),
            };
            format!("{} = {}", role.etiquette(), texte)
        })
        .collect::<Vec<_>>()
        .join(" ; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bilan(crete_mic: u32, octets_mic: u64, crete_sys: u32, octets_sys: u64) -> Bilan {
        Bilan {
            micro: BilanPiste {
                role: Role::Micro,
                octets: octets_mic,
                crete: crete_mic,
                jetes: 0,
                erreur: None,
                fiche: None,
            },
            systeme: BilanPiste {
                role: Role::Systeme,
                octets: octets_sys,
                crete: crete_sys,
                jetes: 0,
                erreur: None,
                fiche: None,
            },
        }
    }

    #[test]
    fn message_reprend_la_sortie_du_micro() {
        let msg = startup_error("connection refused", "");
        assert!(msg.contains("micro : connection refused"), "{msg}");
        assert!(!msg.contains("son systeme"), "{msg}");
    }

    #[test]
    fn message_distingue_les_deux_pistes() {
        let msg = startup_error("pas de source", "pas de monitor");
        assert!(msg.contains("micro : pas de source"), "{msg}");
        assert!(msg.contains("son systeme : pas de monitor"), "{msg}");
    }

    #[test]
    fn constat_identique_annonce_une_seule_fois() {
        let msg = startup_error("meme erreur", "meme erreur");
        assert_eq!(msg.matches("meme erreur").count(), 1, "{msg}");
    }

    #[test]
    fn sans_constat_le_message_dit_quoi_verifier() {
        // Le piege d'origine : inventer « PipeWire indisponible ? » sans rien savoir.
        let msg = startup_error("", "");
        assert!(msg.contains("sans message"), "{msg}");
        assert!(msg.contains("serveur audio"), "{msg}");
    }

    #[test]
    fn constat_long_borne() {
        let long = "x".repeat(1000);
        let msg = startup_error(&long, "");
        assert!(
            msg.chars().count() < 500,
            "message trop long: {}",
            msg.chars().count()
        );
    }

    #[test]
    fn une_piste_pleine_de_zeros_est_muette() {
        // Le symptome d'un tap macOS sans autorisation TCC : des octets, mais tous nuls.
        assert_eq!(bilan(0, 320_000, 4200, 320_000).muette_code(), Some("mic"));
        assert_eq!(bilan(4200, 320_000, 0, 320_000).muette_code(), Some("system"));
        assert_eq!(bilan(0, 320_000, 0, 320_000).muette_code(), Some("both"));
    }

    #[test]
    fn une_piste_avec_du_son_n_est_pas_muette() {
        // Meme tres bas : un enregistrement calme n'est pas une piste sans autorisation.
        assert_eq!(bilan(1, 320_000, 1, 320_000).muette_code(), None);
    }

    #[test]
    fn une_piste_vide_n_est_pas_declaree_muette() {
        // Zero octet, c'est une piste qui n'a pas demarre — un autre probleme, deja dit
        // par `startup_error`. La confondre avec un silence brouillerait le diagnostic.
        assert!(!BilanPiste::inconnue(Role::Micro).muette());
        assert_eq!(bilan(0, 0, 0, 0).muette_code(), None);
    }

    #[test]
    fn le_resume_dit_l_appareil_et_la_crete() {
        let mut b = BilanPiste::inconnue(Role::Systeme);
        b.octets = 64_000;
        b.crete = 1234;
        b.fiche = Some(Fiche {
            hote: "pulseaudio".into(),
            appareil: "Monitor of Built-in Audio".into(),
            taux: 48_000,
            canaux: 2,
            format: "I32".into(),
        });
        let r = b.resume();
        assert!(r.contains("son systeme"), "{r}");
        assert!(r.contains("crete 1234"), "{r}");
        assert!(r.contains("48000 Hz"), "{r}");
        assert!(r.contains("pulseaudio"), "{r}");
    }

    #[test]
    fn les_roles_portent_des_codes_stables() {
        // L'interface les traduit : les changer casserait les messages traduits.
        assert_eq!(Role::Micro.code(), "mic");
        assert_eq!(Role::Systeme.code(), "system");
        assert_eq!(Role::Micro.fichier(), "mic.raw");
        assert_eq!(Role::Systeme.fichier(), "system.raw");
    }

    /// Enregistre vraiment 2 s sur cette machine. Ignore par defaut : il demande du
    /// materiel audio, ce qu'un runner de CI n'a pas.
    ///
    /// A lancer a la main apres toute retouche de la capture :
    /// `cargo test --lib capture_reelle -- --ignored --nocapture`
    #[test]
    #[ignore = "demande une carte son"]
    fn capture_reelle() {
        // La meme resolution d'appareils que celle qui accompagne les erreurs.
        println!("fiche : {}", fiche_audio());
        let dir = std::env::temp_dir().join(format!("cockpit-capture-{}", std::process::id()));
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let bilan = rt.block_on(async {
            let handles = start_capture(&dir).await.expect("demarrage");
            let (mic, sys) = handles.alive_tracks();
            println!("pistes vivantes : micro={mic} systeme={sys}");
            if !mic || !sys {
                println!("constat : {}", handles.startup_error());
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            handles.stop().await
        });
        for piste in [&bilan.micro, &bilan.systeme] {
            println!("{}", piste.resume());
            // Frequence dominante estimee par comptage des passages par zero : avec un
            // son connu qui joue a cote, cette ligne doit l'annoncer. C'est ce qui
            // distingue « des octets sont arrives » de « le son est juste ».
            // NE RIEN FAIRE ENTENDRE POUR CA : router un ton vers un sink nul et capter
            // son monitor (recette dans les Pieges connus du CLAUDE.md) — les enceintes
            // de quelqu'un ne sont pas un banc de test.
            let pcm = std::fs::read(dir.join(piste.role.fichier())).unwrap_or_default();
            let ech: Vec<i16> = pcm
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            let passages = ech.windows(2).filter(|p| (p[0] >= 0) != (p[1] >= 0)).count();
            if !ech.is_empty() {
                println!(
                    "  {} : ~{:.0} Hz dominants",
                    piste.role.etiquette(),
                    passages as f64 * super::super::wav::SAMPLE_RATE as f64 / (2.0 * ech.len() as f64)
                );
            }
        }
        let attendu = 2 * super::super::wav::BYTES_PER_SEC as u64;
        for piste in [&bilan.micro, &bilan.systeme] {
            assert!(
                piste.octets > attendu / 2,
                "{} : {} octets pour ~{} attendus",
                piste.role.etiquette(),
                piste.octets,
                attendu
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
