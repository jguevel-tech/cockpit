//! Le service se prouve SEUL : rien ici ne construit l'application.
//!
//! Deux formes d'essai, et les deux comptent :
//! - le service dans un thread du test, pour tout ce qui se verifie vite ;
//! - le service dans un VRAI processus detache, pour la promesse du produit — le shell
//!   survit au depart du client.
//!
//! Chaque essai a son socket, dans un dossier a lui, efface a la fin : rien ne touche au
//! service de l'utilisateur, ni a ses terminaux.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::client::Client;
use super::protocole::{Pousse, Taille};
// Seuls les essais qui passent par un shell POSIX s'en servent (voir la garde plus bas).
#[cfg(unix)]
use super::protocole::{ActionRecherche, Position};
use super::{serveur, tuyau};
// `lancement` ne sert qu'au banc detache, reserve aux systemes a shell POSIX.
#[cfg(unix)]
use super::lancement;
use crate::terminal::ecran::Ecran;

const TAILLE: Taille = Taille { colonnes: 80, lignes: 24 };
const PATIENCE: Duration = Duration::from_secs(30);

/// Un service qui tourne, et de quoi lui parler.
struct Banc {
    dossier: std::path::PathBuf,
    chemin: std::path::PathBuf,
    fil: Option<std::thread::JoinHandle<()>>,
}

impl Banc {
    /// Un service dans un thread du test.
    fn neuf(historique: usize) -> Self {
        let (dossier, chemin) = emplacement("banc");
        let pour_le_fil = chemin.clone();
        let fil = std::thread::spawn(move || {
            serveur::servir(&pour_le_fil, historique).expect("le service doit pouvoir ecouter");
        });
        attendre_le_socket(&chemin);
        Self { dossier, chemin, fil: Some(fil) }
    }

    fn client(&self) -> (Arc<Client>, Receiver<Pousse>) {
        connecter(&self.chemin)
    }
}

impl Drop for Banc {
    fn drop(&mut self) {
        if let Ok(client) = Client::connecter(&self.chemin, |_| {}) {
            let _ = client.arreter();
        }
        if let Some(fil) = self.fil.take() {
            let _ = fil.join();
        }
        let _ = std::fs::remove_dir_all(&self.dossier);
    }
}

/// Un dossier et un chemin de tuyau propres a cet essai.
///
/// Le dossier sert au menage ; le TUYAU, lui, ne vit pas forcement dedans. Sous Windows un
/// socket local est un tuyau nomme : il se nomme dans l'espace `\\.\pipe\` et n'existe pas
/// dans le systeme de fichiers. Un chemin de fichier y est refuse par `interprocess` avec
/// « not a named pipe path » — deux essais tombaient dessus sur le runner de la v0.38.0.
/// Le code de production le savait deja (`tuyau::chemin`), les essais non.
fn emplacement(quoi: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    static SUIVANT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let numero = SUIVANT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let nom = format!("cockpit-essai-{quoi}-{}-{numero}", std::process::id());
    let dossier = std::env::temp_dir().join(&nom);
    let _ = std::fs::create_dir_all(&dossier);
    #[cfg(unix)]
    let chemin = dossier.join("terminaux.sock");
    #[cfg(windows)]
    let chemin = std::path::PathBuf::from(format!(r"\\.\pipe\{nom}"));
    (dossier, chemin)
}

fn attendre_le_socket(chemin: &std::path::Path) {
    let debut = Instant::now();
    while tuyau::connecter(chemin).is_err() {
        assert!(debut.elapsed() < PATIENCE, "le service n'ecoute toujours pas sur {}", chemin.display());
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn connecter(chemin: &std::path::Path) -> (Arc<Client>, Receiver<Pousse>) {
    let (tx, rx): (Sender<Pousse>, Receiver<Pousse>) = channel();
    let client = Client::connecter(chemin, move |pousse| {
        let _ = tx.send(pousse);
    })
    .expect("connexion au service");
    (client, rx)
}

/// Commandes dont la SORTIE porte un marqueur que la ligne de commande echoee ne contient
/// pas. Sans cette precaution, un test croit avoir vu le resultat alors qu'il n'a vu que
/// ce qui vient d'etre tape — et compare deux ecrans pris a des moments differents.
#[cfg(unix)]
const MARQUEUR_BONJOUR: &[u8] = b"printf 'bonjour%s\\n' -du-service\r";
#[cfg(unix)]
const MARQUEUR_TRACE: &[u8] = b"printf 'trace%s\\n' -avant-la-coupure\r";
#[cfg(unix)]
const MARQUEUR_AIGUILLE: &[u8] = b"printf 'aiguille%s\\n' -unique\r";

fn dossier_de_travail() -> String {
    std::env::temp_dir().to_string_lossy().to_string()
}

/// Rejoue tout ce que le service pousse dans un emulateur neuf : c'est ce que fait le
/// terminal du frontend. Rend l'emulateur et ce qui reste a lire.
struct Miroir {
    ecran: Ecran,
}

impl Miroir {
    fn neuf() -> Self {
        Self { ecran: Ecran::nouveau(TAILLE.colonnes as usize, TAILLE.lignes as usize) }
    }

    fn avaler(&mut self, pousse: &Pousse) {
        match pousse {
            Pousse::Sortie { octets, .. } | Pousse::Redessin { octets, .. } => {
                self.ecran.avaler(octets)
            }
            _ => {}
        }
    }

    fn texte(&self) -> String {
        self.ecran.texte_region((0, 0), (TAILLE.lignes as i32 - 1, TAILLE.colonnes - 1))
    }
}

/// Ecoule ce que le service pousse jusqu'a ce que le miroir montre ce qu'on attend.
fn attendre_a_l_ecran(miroir: &mut Miroir, recu: &Receiver<Pousse>, motif: &str) {
    let debut = Instant::now();
    loop {
        if miroir.texte().contains(motif) {
            return;
        }
        assert!(
            debut.elapsed() < PATIENCE,
            "« {motif} » n'est jamais apparu. Ecran :\n{}",
            miroir.texte()
        );
        match recu.recv_timeout(Duration::from_millis(200)) {
            Ok(pousse) => miroir.avaler(&pousse),
            Err(_) => continue,
        }
    }
}

/// Vide ce qui reste a recevoir, une fois le shell au repos.
fn ecouler(miroir: &mut Miroir, recu: &Receiver<Pousse>) {
    while let Ok(pousse) = recu.recv_timeout(Duration::from_millis(300)) {
        miroir.avaler(&pousse);
    }
}

// --- Le tour complet ---

/// Creer, ecrire, lire la sortie, redimensionner, fermer. Le minimum vital, par le socket.
/// Les essais ci-dessous TAPENT dans le shell de la machine, et se reperent dans sa
/// sortie. Ils sont donc reserves aux systemes ou ce shell est un shell POSIX : sous
/// Windows c'est `%COMSPEC%`, soit `cmd.exe`, qui ne connait ni `printf`, ni `cat`, ni
/// `for i in $(seq 1 400)`.
///
/// Ce qui reste couvert partout : le protocole, la poignee de main versionnee, le refus
/// d'un interlocuteur etranger, la reconciliation, la commande d'ouverture, et tout
/// l'emulateur (`ecran/tests.rs`). Ce qui n'est couvert NULLE PART sous Windows : le
/// comportement d'un vrai shell dans un ConPTY. Aucun essai ne peut le dire ici — cela
/// demande une machine Windows, et le premier installeur produit par la CI est ce qui
/// permettra de le savoir.
///
/// Ne PAS remplacer ces commandes par des equivalents `cmd.exe` inventes sans machine
/// pour les essayer : un essai vert qu'on n'a jamais vu tourner ne prouve rien, et le
/// marqueur guette ne doit surtout pas apparaitre dans la LIGNE TAPEE (le PTY en renvoie
/// l'echo avant que le shell ne l'execute — voir les Pieges connus).
#[cfg(unix)]
#[test]
fn le_tour_complet_par_le_socket() {
    let banc = Banc::neuf(500);
    let (client, recu) = banc.client();
    let mut miroir = Miroir::neuf();

    client.creer(1, &dossier_de_travail(), TAILLE, None).unwrap();
    client.attacher(1, TAILLE).unwrap();
    client.ecrire(1, MARQUEUR_BONJOUR).unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "bonjour-du-service");

    // Le redimensionnement se voit dans le service : la session porte la nouvelle taille.
    client.redimensionner(1, Taille { colonnes: 100, lignes: 30 }).unwrap();
    let sessions = client.lister().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, 1);
    assert!(sessions[0].vivant);
    assert_eq!(sessions[0].taille, Taille { colonnes: 100, lignes: 30 });

    client.fermer(1).unwrap();
    let debut = Instant::now();
    while !client.lister().unwrap().is_empty() {
        assert!(debut.elapsed() < PATIENCE, "le terminal ferme reste dans la liste");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// La commande d'ouverture (bouton « ▶ Cmd », palette, shell de conteneur).
#[test]
fn la_commande_d_ouverture_est_lancee() {
    let banc = Banc::neuf(500);
    let (client, recu) = banc.client();
    let mut miroir = Miroir::neuf();
    client
        .creer(1, &dossier_de_travail(), TAILLE, Some("echo commande-d-ouverture".into()))
        .unwrap();
    client.attacher(1, TAILLE).unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "commande-d-ouverture");
}

/// Une operation sur un terminal inconnu doit dire pourquoi, pas echouer en silence.
#[test]
fn un_terminal_inconnu_donne_une_erreur_lisible() {
    let banc = Banc::neuf(500);
    let (client, _recu) = banc.client();
    let erreur = client.attacher(404, TAILLE).unwrap_err();
    assert!(erreur.contains("404") && erreur.contains("inconnu"), "{erreur}");
}

// --- La promesse du produit : la survie ---

/// LE test qui compte. Un service dans un VRAI processus detache : on cree un terminal, on
/// y lance un programme, on TUE le client, on se reconnecte, et l'ecran est le meme.
///
/// Le service est lance par le meme chemin que dans l'application (`lancer_detache`, donc
/// double fork + setsid), simplement depuis le binaire de test au lieu de celui de
/// Cockpit : c'est le mecanisme de detachement qui est verifie, pas seulement le protocole.
#[cfg(unix)]
#[test]
fn le_shell_survit_a_la_mort_du_client() {
    let banc = BancDetache::neuf("survie");
    let chemin = banc.chemin.clone();

    // Il est bien DETACHE : son parent n'est plus le processus qui l'a lance.
    verifier_qu_il_est_detache(&chemin);

    // Premiere vie du client.
    let avant = {
        let (client, recu) = connecter(&chemin);
        let mut miroir = Miroir::neuf();
        client.creer(1, &dossier_de_travail(), TAILLE, None).unwrap();
        client.attacher(1, TAILLE).unwrap();
        client.ecrire(1, MARQUEUR_TRACE).unwrap();
        attendre_a_l_ecran(&mut miroir, &recu, "trace-avant-la-coupure");
        ecouler(&mut miroir, &recu);
        miroir.texte()
    }; // le client tombe ici : socket ferme, exactement comme une application qui se ferme

    // Le shell continue de tourner pendant que plus personne n'est connecte.
    std::thread::sleep(Duration::from_millis(300));

    // Seconde vie.
    let (client, recu) = connecter(&chemin);
    let sessions = client.lister().unwrap();
    assert_eq!(sessions.len(), 1, "le terminal aurait du survivre au depart du client");
    assert!(sessions[0].vivant);

    let mut miroir = Miroir::neuf();
    client.attacher(1, TAILLE).unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "trace-avant-la-coupure");
    ecouler(&mut miroir, &recu);
    assert_eq!(miroir.texte(), avant, "l'ecran retrouve n'est pas celui qu'on avait quitte");

    // Et il repond toujours.
    client.ecrire(1, b"echo toujours-vivant\r").unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "toujours-vivant");

}

/// Un service dans un VRAI processus detache, arrete quoi qu'il arrive.
///
/// Le `Drop` compte : sans lui, un essai qui echoue laisse un service et ses shells
/// tourner jusqu'a la prochaine deconnexion de l'utilisateur (constate le 2026-08-21).
#[cfg(unix)]
struct BancDetache {
    dossier: std::path::PathBuf,
    chemin: std::path::PathBuf,
}

#[cfg(unix)]
impl BancDetache {
    fn neuf(quoi: &str) -> Self {
        let (dossier, chemin) = emplacement(quoi);
        lancement::demarrer_avec(&chemin, || service_dans_le_binaire_de_test(&chemin))
            .expect("le service detache doit demarrer");
        Self { dossier, chemin }
    }
}

#[cfg(unix)]
impl Drop for BancDetache {
    fn drop(&mut self) {
        if let Ok(client) = Client::connecter(&self.chemin, |_| {}) {
            let _ = client.arreter();
        }
        let _ = std::fs::remove_dir_all(&self.dossier);
    }
}

/// Le double fork a-t-il fait son travail ? Le service ne doit plus etre notre enfant.
///
/// PIEGE : sur un bureau Linux moderne, l'orphelin n'est PAS adopte par le pid 1 mais par
/// le `systemd --user` de la session, qui se declare sous-moissonneur. Un essai qui
/// exigerait `ppid == 1` echouerait alors qu'il n'y a rien a corriger.
#[cfg(all(unix, target_os = "linux"))]
fn verifier_qu_il_est_detache(chemin: &std::path::Path) {
    use interprocess::local_socket::traits::StreamCommon as _;
    let flux = tuyau::connecter(chemin).expect("connexion au service");
    let pid = flux.peer_creds().expect("identite du pair").pid().expect("pid du pair");
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).expect("stat du service");
    // Le nom du process est entre parentheses et peut contenir des espaces : on repart de
    // la derniere parenthese fermante, le ppid est le 2e champ apres elle.
    let apres = &stat[stat.rfind(')').expect("nom du process") + 1..];
    let ppid: u32 = apres.split_whitespace().nth(1).expect("ppid").parse().expect("ppid");
    assert_ne!(
        ppid,
        std::process::id(),
        "le service est reste un enfant du processus qui l'a lance : le double fork n'a pas eu lieu"
    );
}

#[cfg(all(unix, not(target_os = "linux")))]
fn verifier_qu_il_est_detache(_chemin: &std::path::Path) {}

/// Une application plein ecran (vim) doit se retrouver DESSINEE et VIVANTE apres une
/// reconnexion. C'est le cas ou le redessin ne peut pas se contenter du flux : l'ecran
/// alternatif n'existe que dans la memoire du service.
#[test]
fn une_application_plein_ecran_est_retrouvee_vivante() {
    let Some(vim) = trouver("vim") else {
        eprintln!("vim absent : essai plein ecran saute");
        return;
    };
    let banc = Banc::neuf(500);

    {
        let (client, recu) = banc.client();
        let mut miroir = Miroir::neuf();
        client.creer(1, &dossier_de_travail(), TAILLE, None).unwrap();
        client.attacher(1, TAILLE).unwrap();
        client
            .ecrire(1, format!("{vim} -n -u NONE -c 'set noswapfile' /tmp/essai-cockpit.txt\r").as_bytes())
            .unwrap();
        attendre_a_l_ecran(&mut miroir, &recu, "essai-cockpit.txt");
        // On tape du texte DANS vim : ce qui suit doit le retrouver.
        client.ecrire(1, b"ivim-tourne-toujours").unwrap();
        attendre_a_l_ecran(&mut miroir, &recu, "vim-tourne-toujours");
    }

    // Nouveau client : l'ecran alternatif de vim doit revenir dessine.
    let (client, recu) = banc.client();
    let mut miroir = Miroir::neuf();
    client.attacher(1, TAILLE).unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "vim-tourne-toujours");

    // Et vim REPOND encore : on lui fait ecrire autre chose.
    client.ecrire(1, b" et-il-repond").unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "et-il-repond");

    // On le quitte proprement pour ne pas laisser un vim en fond.
    client.ecrire(1, b"\x1b:q!\r").unwrap();
    let debut = Instant::now();
    while miroir.texte().contains("vim-tourne-toujours") && debut.elapsed() < Duration::from_secs(10) {
        if let Ok(p) = recu.recv_timeout(Duration::from_millis(200)) {
            miroir.avaler(&p);
        }
    }
    let _ = client.fermer(1);
}

/// Se reconnecter PENDANT que le shell ecrit ne doit ni perdre ni dedoubler ce qui
/// s'affiche. L'invariant se verifie a l'ETAT : ce qu'un terminal neuf montre apres avoir
/// rejoue tout ce qu'il a recu doit etre exactement ce que le service affiche.
#[cfg(unix)]
#[test]
fn une_reconnexion_en_plein_flux_laisse_l_ecran_juste() {
    let banc = Banc::neuf(2000);
    let (client, recu) = banc.client();
    client.creer(1, &dossier_de_travail(), TAILLE, None).unwrap();
    client.attacher(1, TAILLE).unwrap();

    // Une sortie longue et reguliere : on va couper au milieu.
    client
        .ecrire(1, b"for i in $(seq 1 400); do echo ligne-$i; done; echo FINI-LE-FLUX\r")
        .unwrap();
    std::thread::sleep(Duration::from_millis(60));
    drop(recu);
    drop(client);

    // On revient au milieu du flux.
    let (client, recu) = banc.client();
    let mut miroir = Miroir::neuf();
    client.attacher(1, TAILLE).unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "FINI-LE-FLUX");
    ecouler(&mut miroir, &recu);

    // La verite : l'etat complet redemande au service. Si le flux avait perdu ou double
    // quoi que ce soit, les deux ecrans differeraient.
    let (temoin_tx, temoin_rx) = channel();
    let temoin_client = Client::connecter(&banc.chemin, move |p| {
        let _ = temoin_tx.send(p);
    })
    .unwrap();
    temoin_client.redessiner(1, false).unwrap();
    let mut temoin = Miroir::neuf();
    let recu_temoin = temoin_rx.recv_timeout(PATIENCE).expect("le redessin demande");
    temoin.avaler(&recu_temoin);

    assert_eq!(
        miroir.texte(),
        temoin.texte(),
        "le flux recu ne reconstitue pas l'ecran du service"
    );
}

// --- Recherche et copie ---

#[cfg(unix)]
#[test]
fn la_recherche_et_la_copie_passent_par_le_socket() {
    let banc = Banc::neuf(2000);
    let (client, recu) = banc.client();
    let mut miroir = Miroir::neuf();
    client.creer(1, &dossier_de_travail(), TAILLE, None).unwrap();
    client.attacher(1, TAILLE).unwrap();
    client.ecrire(1, MARQUEUR_AIGUILLE).unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "aiguille-unique");
    client.ecrire(1, b"seq 1 60\r").unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "60");
    ecouler(&mut miroir, &recu);

    let (total, index, ou) = client.chercher(1, ActionRecherche::Demarrer, "aiguille-unique").unwrap();
    assert!(total >= 1, "total={total}");
    assert_eq!(index, Some(total - 1));
    let ou = ou.expect("une position");
    assert!(ou.ligne < 0, "l'occurrence a defile hors de l'ecran : {ou:?}");

    // La copie d'une region : un seul appel, la ou tmux en demandait cinq.
    let texte = client
        .copier_selection(1, ou, Position { ligne: ou.ligne, colonne: ou.colonne + 14 })
        .unwrap();
    assert_eq!(texte, "aiguille-unique");

    // Annuler ne doit pas etre une erreur, meme sans recherche en cours.
    client.chercher(1, ActionRecherche::Annuler, "").unwrap();
}

// --- Poignee de main et proprietaire ---

/// Un service d'une autre version doit se reconnaitre AVANT tout echange, et le dire avec
/// les deux numeros. Ici on simule le service ancien en repondant un preambule d'une
/// version differente.
#[test]
fn un_service_d_une_autre_version_est_reconnu() {
    use super::protocole::{ErreurPoignee, VERSION};
    use interprocess::local_socket::traits::Listener as _;
    use std::io::Write;

    let (dossier, chemin) = emplacement("version");
    let ecoute = tuyau::ecouter(&chemin).unwrap();
    let faux = std::thread::spawn(move || {
        if let Ok(flux) = ecoute.accept() {
            // Un preambule valide, mais d'une version anterieure.
            let mut brut = b"CKPTERM\0".to_vec();
            brut.extend_from_slice(&(VERSION.saturating_sub(1)).to_be_bytes());
            let _ = (&flux).write_all(&brut);
            std::thread::sleep(Duration::from_millis(200));
        }
    });

    let erreur = Client::connecter(&chemin, |_| {}).err().expect("la connexion doit etre refusee");
    match erreur {
        ErreurPoignee::ServiceTropAncien { service, application } => {
            assert_eq!(application, VERSION);
            assert_eq!(service, VERSION - 1);
        }
        // Si VERSION vaut 1, la version « anterieure » est 0 et le message reste le meme.
        autre => panic!("attendu un desaccord de version, recu {autre:?}"),
    }
    let _ = faux.join();
    let _ = std::fs::remove_dir_all(&dossier);
}

/// Quelque chose ecoute, mais ce n'est pas nous : il faut le dire, pas se taire.
#[test]
fn un_interlocuteur_etranger_est_refuse() {
    use super::protocole::ErreurPoignee;
    use interprocess::local_socket::traits::Listener as _;
    use std::io::Write;

    let (dossier, chemin) = emplacement("etranger");
    let ecoute = tuyau::ecouter(&chemin).unwrap();
    let faux = std::thread::spawn(move || {
        if let Ok(flux) = ecoute.accept() {
            let _ = (&flux).write_all(b"HTTP/1.1 200 OK\r\n\r\n");
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    assert_eq!(
        Client::connecter(&chemin, |_| {}).err(),
        Some(ErreurPoignee::PasNotreService)
    );
    let _ = faux.join();
    let _ = std::fs::remove_dir_all(&dossier);
}

// --- Les deux chiffres a ne pas degrader ---

/// La latence de frappe : ce que coute le service entre la touche et l'echo du shell.
/// tmux ajoutait 0,4 ms. Le chiffre affiche par `--nocapture` est celui qui compte ; le
/// seuil du test est large a dessein (il tourne aussi en debug, et sur des machines
/// chargees).
#[cfg(unix)]
#[test]
fn la_latence_de_frappe_reste_sous_celle_de_tmux() {
    const TOURS: usize = 200;
    let banc = Banc::neuf(500);
    let (client, recu) = banc.client();
    let mut miroir = Miroir::neuf();
    client.creer(1, &dossier_de_travail(), TAILLE, None).unwrap();
    client.attacher(1, TAILLE).unwrap();
    // `cat` renvoie ce qu'on lui donne : l'echo ne depend plus de l'invite du shell.
    client.ecrire(1, b"cat\r").unwrap();
    attendre_a_l_ecran(&mut miroir, &recu, "cat");
    ecouler(&mut miroir, &recu);

    let mut poser = Vec::with_capacity(TOURS);
    let mut boucle = Vec::with_capacity(TOURS);
    for _ in 0..TOURS {
        let debut = Instant::now();
        client.ecrire(1, b"x").unwrap();
        poser.push(debut.elapsed());
        // On attend que l'octet soit revenu affiche.
        loop {
            match recu.recv_timeout(PATIENCE) {
                Ok(Pousse::Sortie { octets, .. }) if octets.contains(&b'x') => break,
                Ok(_) => continue,
                Err(_) => panic!("pas d'echo"),
            }
        }
        boucle.push(debut.elapsed());
    }

    // Le meme aller-retour sans le service : un PTY nu, pour isoler ce que NOUS ajoutons.
    let nu = aller_retour_sur_un_pty_nu(TOURS);

    let mediane = |mut v: Vec<Duration>| {
        v.sort();
        v[v.len() / 2]
    };
    let (poser, boucle, nu) = (mediane(poser), mediane(boucle), mediane(nu));
    eprintln!(
        "frappe : depot sur le socket {:?} | aller-retour complet {:?} | PTY nu {:?} | \
         surcout du service {:?}",
        poser,
        boucle,
        nu,
        boucle.saturating_sub(nu)
    );
    // tmux coutait 0,4 ms par frappe. On se donne dix fois cette marge pour que le test
    // ne clignote pas sur une machine chargee : ce qu'il attrape, c'est un effondrement
    // (un regroupement pose sur le chemin de frappe, un aller-retour ajoute), pas une
    // variation de quelques dizaines de microsecondes.
    assert!(
        boucle.saturating_sub(nu) < Duration::from_millis(4),
        "le service ajoute {:?} par frappe",
        boucle.saturating_sub(nu)
    );
    let _ = client.fermer(1);
}

/// L'aller-retour de reference : ecrire dans un PTY et relire l'echo, sans rien autour.
#[cfg(unix)]
fn aller_retour_sur_un_pty_nu(tours: usize) -> Vec<Duration> {
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::io::{Read, Write};

    let paire = native_pty_system()
        .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
        .unwrap();
    let mut enfant = paire.slave.spawn_command(CommandBuilder::new("cat")).unwrap();
    drop(paire.slave);
    let mut lecteur = paire.master.try_clone_reader().unwrap();
    let mut ecrivain = paire.master.take_writer().unwrap();

    let mut mesures = Vec::with_capacity(tours);
    let mut tampon = [0u8; 1024];
    for _ in 0..tours {
        let debut = Instant::now();
        ecrivain.write_all(b"x").unwrap();
        ecrivain.flush().unwrap();
        loop {
            let lus = lecteur.read(&mut tampon).unwrap();
            if tampon[..lus].contains(&b'x') {
                break;
            }
        }
        mesures.push(debut.elapsed());
    }
    let _ = enfant.kill();
    let _ = enfant.wait();
    mesures
}

/// Ce qu'une session coute en memoire : a vide, puis l'historique plein, a deux largeurs.
///
/// C'est ce test qui a decide du plafond en CELLULES (`serveur::CELLULES_D_HISTORIQUE`) :
/// compter en lignes seulement laissait la facture suivre la largeur de la fenetre.
#[test]
fn le_cout_memoire_d_une_session_est_mesure() {
    if memoire_resident().is_none() {
        eprintln!("memoire resident non lisible sur ce systeme");
        return;
    }
    let banc = Banc::neuf(serveur::HISTORIQUE);
    let (client, recu) = banc.client();
    let mut miroir = Miroir::neuf();

    // Une premiere session paie les tampons communs : on mesure les SUIVANTES, c'est
    // elles qui comptent quand un utilisateur ouvre son onzieme terminal.
    client.creer(1, &dossier_de_travail(), TAILLE, None).unwrap();
    client.attacher(1, TAILLE).unwrap();
    ecouler(&mut miroir, &recu);

    for (numero, colonnes) in [(2i64, 80u16), (3, 240)] {
        let taille = Taille { colonnes, lignes: 24 };
        let lignes = serveur::lignes_d_historique(colonnes, serveur::HISTORIQUE);
        let avant = memoire_resident().unwrap();
        client.creer(numero, &dossier_de_travail(), taille, None).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        let a_vide = memoire_resident().unwrap().saturating_sub(avant);

        let avant_plein = memoire_resident().unwrap();
        client.ecrire(numero, format!("seq 1 {}\r", lignes + 100).as_bytes()).unwrap();
        attendre_la_stabilite();
        let plein = memoire_resident().unwrap().saturating_sub(avant_plein);
        eprintln!(
            "session de plus, {colonnes} colonnes / {lignes} lignes d'historique : \
             {} Ko a vide, +{} Ko une fois pleine (total {} Ko)",
            a_vide / 1024,
            plein / 1024,
            (a_vide + plein) / 1024
        );
    }
}

/// Attend que la memoire du processus cesse de bouger : le shell finit d'ecrire, le
/// service finit d'avaler.
fn attendre_la_stabilite() {
    let debut = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(300));
        let temoin = memoire_resident().unwrap_or(0);
        std::thread::sleep(Duration::from_millis(500));
        if memoire_resident().unwrap_or(0) == temoin || debut.elapsed() > Duration::from_secs(90) {
            return;
        }
    }
}

fn memoire_resident() -> Option<usize> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: usize = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}

// --- Outillage ---

fn trouver(programme: &str) -> Option<String> {
    let chemins = std::env::var_os("PATH")?;
    std::env::split_paths(&chemins)
        .map(|d| d.join(programme))
        .find(|c| c.is_file())
        .map(|c| c.to_string_lossy().into_owned())
}

/// Variable qui transforme un processus du binaire de TEST en service.
const VARIABLE_ESSAI: &str = "COCKPIT_ESSAI_SERVICE_SOCKET";

/// La commande qui relance le binaire de test en service, via le point d'entree
/// `ce_processus_est_le_service` ci-dessous.
#[cfg(unix)]
fn service_dans_le_binaire_de_test(
    chemin: &std::path::Path,
) -> Result<std::process::Command, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut commande = std::process::Command::new(exe);
    commande
        .arg("--exact")
        .arg("terminal::service::tests::ce_processus_est_le_service")
        .arg("--nocapture")
        .env(VARIABLE_ESSAI, chemin);
    Ok(commande)
}

/// Point d'entree du service quand le binaire de test est relance avec la variable
/// ci-dessus. Sans elle — c'est-a-dire pendant un `cargo test` ordinaire — il ne fait
/// rien : c'est un essai qui passe en zero seconde.
#[test]
fn ce_processus_est_le_service() {
    let Some(chemin) = std::env::var_os(VARIABLE_ESSAI) else { return };
    let chemin = std::path::PathBuf::from(chemin);
    serveur::servir(&chemin, 500).expect("le service doit pouvoir ecouter");
}

/// Lancer le service NE DOIT PAS laisser de zombie.
///
/// Le lancement detache fait deux `fork` : celui de `Command::spawn` (l'intermediaire) et
/// celui de `pre_exec` (le service). L'intermediaire s'efface aussitot pour que le service
/// soit adopte, et c'est `lancer_detache` qui le ramasse par `wait`. Oublier ce `wait`
/// laisserait un `[cockpit] <defunct>` par lancement, invisible et sans consequence — donc
/// jamais remarque.
///
/// Ce que cet essai FERME comme piste : des `[cockpit] <defunct>` ont ete observes sous
/// l'application le 2026-08-21 et attribues a ce code. Mesure faite : zero, dix fois de
/// suite. Ils viennent d'ailleurs — c'est le patron de `g_spawn` de GLib, qui fait lui aussi
/// un fork intermediaire, et un intermediaire non ramasse porte le nom du programme parce
/// qu'il n'a jamais `exec`. Ne pas rechercher la cause dans notre lancement.
#[cfg(target_os = "linux")]
#[test]
fn lancer_le_service_ne_laisse_pas_de_zombie() {
    let banc = BancDetache::neuf("zombie");
    // Le service tourne : l'intermediaire a donc vecu et devrait etre ramasse.
    std::thread::sleep(Duration::from_millis(300));
    let restes = zombies_de_ce_processus();
    drop(banc);
    assert!(
        restes.is_empty(),
        "{} enfant(s) non ramasse(s) apres le lancement du service : {restes:?}",
        restes.len()
    );
}

/// Les enfants zombies de CE processus QUI N'ONT JAMAIS `exec` — donc les intermediaires
/// de fork perdus, et rien d'autre.
///
/// Le filtre sur le nom est ce qui rend la mesure utilisable : les essais voisins lancent de
/// vrais shells, et un shell qui vient de mourir passe par l'etat `Z` une fraction de seconde
/// avant que son thread lecteur ne le ramasse. Les compter tous ferait echouer cet essai au
/// hasard de l'ordonnancement — c'est arrive des le premier passage en parallele. Un shell
/// porte son propre nom (`zsh`, `sh`, `cmd`) ; l'intermediaire du double fork, lui, porte
/// LE NOTRE, precisement parce qu'il n'a jamais `exec`.
///
/// `/proc/<pid>/stat` plutot qu'un `ps` : pas de processus externe, et l'etat, le parent et
/// le nom se lisent dans le meme fichier. Le champ `comm` peut contenir des espaces et des
/// parentheses, d'ou la lecture APRES la derniere parenthese fermante.
#[cfg(target_os = "linux")]
fn zombies_de_ce_processus() -> Vec<i32> {
    fn nom_et_reste(stat: &str) -> Option<(String, Vec<&str>)> {
        let debut = stat.find('(')?;
        let fin = stat.rfind(')')?;
        let nom = stat.get(debut + 1..fin)?.to_string();
        Some((nom, stat.get(fin + 1..)?.split_whitespace().collect()))
    }

    let moi = std::process::id().to_string();
    let Ok(mon_stat) = std::fs::read_to_string("/proc/self/stat") else { return Vec::new() };
    let Some((mon_nom, _)) = nom_et_reste(&mon_stat) else { return Vec::new() };
    let Ok(entrees) = std::fs::read_dir("/proc") else { return Vec::new() };
    entrees
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.parse::<i32>().ok())
        .filter(|pid| {
            let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
                return false;
            };
            let Some((nom, champs)) = nom_et_reste(&stat) else { return false };
            nom == mon_nom
                && champs.first() == Some(&"Z")
                && champs.get(1) == Some(&moi.as_str())
        })
        .collect()
}
