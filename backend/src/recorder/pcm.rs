//! Mise au format d'une piste : ce que le materiel livre -> ce que le pipeline attend.
//!
//! LA FRONTIERE DU MODULE reunion est le fichier `.raw` : PCM **s16le mono 16 kHz**, a
//! l'octet pres. Les chunks de 10 min, la detection de silence, l'en-tete WAV et la
//! fusion Moi/Eux en dependent, et c'est ce qui garde la capture remplacable sans que
//! l'aval s'en apercoive.
//!
//! Le materiel, lui, livre son format natif : 48 000 Hz, deux canaux, entiers 32 bits sur
//! une machine ordinaire (mesure au banc). Le melange des canaux, le reechantillonnage et
//! la conversion en `i16` sont donc NOTRE travail depuis qu'un programme externe ne le
//! fait plus. Ce module ne parle a personne — entree : trames entrelacees en `f32`,
//! sortie : des octets — ce qui le rend entierement testable.

/// Taux d'echantillonnage du fichier `.raw`. Doit rester egal a `wav::SAMPLE_RATE`.
pub const TAUX_CIBLE: u32 = 16_000;

/// Nombre de passages par zero du sinc conserves de chaque cote.
///
/// Fixe la largeur de la bande de transition du filtre, donc l'attenuation obtenue a la
/// frequence de Nyquist de sortie (8 kHz). A 32, un ton de 10 kHz ressort a moins de
/// -46 dB — a la mesure, plus rien du tout (test `un_ton_au_dela_de_nyquist_est_ecrase`),
/// pour ~215 coefficients par
/// echantillon produit — soit 3,4 millions de multiplications par seconde d'audio, ce qui
/// ne se voit pas. Descendre a 16 laisse passer un repliement audible entre 8 et 8,5 kHz.
const LOBES: f64 = 32.0;

/// Frequence de coupure, en fraction du taux de sortie. 0,45 laisse une bande de
/// transition avant 0,5 (Nyquist) sans rogner la parole (7,2 kHz a 16 kHz de sortie).
const COUPURE: f64 = 0.45;

/// Finesse de la table du noyau, en points par echantillon d'entree. Le noyau est lisse :
/// une interpolation lineaire entre deux points de la table suffit, et evite de calculer
/// un `sin` et deux `cos` par coefficient — ce qui, lui, se verrait.
const FINESSE: usize = 64;

/// Melange une trame entrelacee en une seule voie, par moyenne des canaux.
///
/// La moyenne et non la somme : deux canaux identiques doivent rendre le meme niveau, pas
/// le double (qui saturerait).
pub fn melanger_mono(entrelace: &[f32], canaux: u16) -> Vec<f32> {
    let canaux = canaux.max(1) as usize;
    if canaux == 1 {
        return entrelace.to_vec();
    }
    entrelace
        .chunks_exact(canaux)
        .map(|trame| trame.iter().sum::<f32>() / canaux as f32)
        .collect()
}

/// Convertit un flottant -1..1 en `i16`, en BORNANT au lieu de replier.
///
/// Un depassement (un materiel peut livrer plus de 1.0) qui repasserait par un `as i16`
/// changerait de signe : un pic deviendrait un pic inverse, donc un claquement.
fn en_i16(v: f32) -> i16 {
    (v * 32767.0).round().clamp(-32768.0, 32767.0) as i16
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        px.sin() / px
    }
}

/// Reechantillonneur mono par convolution sinc fenetree (fenetre de Blackman).
///
/// Il marche pour un rapport QUELCONQUE — 48 000 -> 16 000 est un facteur 3 exact, mais
/// 44 100 -> 16 000 n'en est pas un, et jeter deux echantillons sur trois ne serait de
/// toute facon pas un reechantillonnage : le contenu au-dela de 8 kHz se replierait dans
/// la bande de la parole.
struct Reechantillonneur {
    /// Echantillons d'entree consommes par echantillon produit.
    pas: f64,
    /// Demi-largeur du noyau, en echantillons d'entree.
    demi: usize,
    /// Noyau tabule sur [-demi, +demi], `FINESSE` points par echantillon d'entree.
    noyau: Vec<f32>,
    /// Echantillons d'entree en attente, contexte gauche compris.
    tampon: Vec<f32>,
    /// Position du prochain echantillon a produire, dans `tampon`.
    position: f64,
}

impl Reechantillonneur {
    fn new(taux_entree: u32) -> Self {
        let pas = taux_entree as f64 / TAUX_CIBLE as f64;
        // En cycles par echantillon d'ENTREE : la coupure suit le plus bas des deux taux.
        let coupure = COUPURE * (1.0 / pas).min(1.0);
        let demi = (LOBES / (2.0 * coupure)).ceil() as usize;

        let points = 2 * demi * FINESSE + 1;
        let mut noyau = Vec::with_capacity(points);
        for j in 0..points {
            let t = j as f64 / FINESSE as f64 - demi as f64;
            let angle = std::f64::consts::PI * t / demi as f64;
            let fenetre = 0.42 + 0.5 * angle.cos() + 0.08 * (2.0 * angle).cos();
            noyau.push((sinc(2.0 * coupure * t) * fenetre) as f32);
        }

        Self {
            pas,
            demi,
            noyau,
            // Le contexte gauche du premier echantillon est du silence : sans ce
            // remplissage, la sortie commencerait `demi` echantillons plus loin et les
            // deux pistes de la reunion seraient decalees l'une par rapport a l'autre.
            tampon: vec![0.0; demi],
            position: demi as f64,
        }
    }

    /// Coefficient du noyau pour un ecart `t` (en echantillons d'entree).
    fn coefficient(&self, t: f64) -> f32 {
        let x = (t + self.demi as f64) * FINESSE as f64;
        if x < 0.0 {
            return 0.0;
        }
        let i = x as usize;
        match (self.noyau.get(i), self.noyau.get(i + 1)) {
            (Some(a), Some(b)) => {
                let frac = (x - i as f64) as f32;
                a + (b - a) * frac
            }
            (Some(a), None) => *a,
            _ => 0.0,
        }
    }

    /// Produit tout ce que le tampon permet, et libere ce qui ne sert plus.
    fn tirer(&mut self, sortie: &mut Vec<f32>) {
        let demi = self.demi as f64;
        while self.position + demi < self.tampon.len() as f64 {
            let debut = (self.position - demi).ceil() as usize;
            let fin = (self.position + demi).floor() as usize;
            let mut somme = 0.0f32;
            let mut poids = 0.0f32;
            for k in debut..=fin.min(self.tampon.len() - 1) {
                let h = self.coefficient(k as f64 - self.position);
                somme += h * self.tampon[k];
                poids += h;
            }
            // Division par la somme des coefficients : le noyau n'est pas echantillonne
            // aux memes endroits d'un echantillon de sortie a l'autre, et sans cette
            // normalisation le gain ondulerait avec la phase.
            sortie.push(if poids.abs() > 1e-9 { somme / poids } else { 0.0 });
            self.position += self.pas;
        }

        let garde = self.position.floor() as isize - self.demi as isize;
        if garde > 0 {
            let garde = (garde as usize).min(self.tampon.len());
            self.tampon.drain(..garde);
            self.position -= garde as f64;
        }
    }

    fn pousser(&mut self, mono: &[f32], sortie: &mut Vec<f32>) {
        self.tampon.extend_from_slice(mono);
        self.tirer(sortie);
    }

    /// Vide la queue en completant le contexte droit par du silence.
    fn fin(&mut self, sortie: &mut Vec<f32>) {
        let queue = vec![0.0; self.demi + 1];
        self.pousser(&queue, sortie);
    }
}

/// Le format du materiel -> les octets du fichier `.raw`.
pub struct Convertisseur {
    canaux: u16,
    /// Absent quand le materiel livre deja 16 kHz : filtrer alors ne ferait que rogner
    /// les aigus sans rien corriger.
    reech: Option<Reechantillonneur>,
}

impl Convertisseur {
    pub fn new(taux_entree: u32, canaux: u16) -> Self {
        Self {
            canaux,
            reech: (taux_entree != TAUX_CIBLE).then(|| Reechantillonneur::new(taux_entree)),
        }
    }

    /// Convertit un lot de trames entrelacees. Peut rendre moins que son entree, ou rien :
    /// le filtre garde de quoi calculer le prochain echantillon.
    pub fn pousser(&mut self, entrelace: &[f32]) -> Vec<u8> {
        let mono = melanger_mono(entrelace, self.canaux);
        match &mut self.reech {
            None => Self::octets(&mono),
            Some(r) => {
                let mut sortie = Vec::new();
                r.pousser(&mono, &mut sortie);
                Self::octets(&sortie)
            }
        }
    }

    /// Ce qui reste dans le filtre a la fin de l'enregistrement.
    pub fn fin(&mut self) -> Vec<u8> {
        match &mut self.reech {
            None => Vec::new(),
            Some(r) => {
                let mut sortie = Vec::new();
                r.fin(&mut sortie);
                Self::octets(&sortie)
            }
        }
    }

    fn octets(mono: &[f32]) -> Vec<u8> {
        let mut out = Vec::with_capacity(mono.len() * 2);
        for v in mono {
            out.extend_from_slice(&en_i16(*v).to_le_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echantillons(octets: &[u8]) -> Vec<i16> {
        octets
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect()
    }

    fn sinusoide(frequence: f64, taux: u32, secondes: f64, canaux: u16) -> Vec<f32> {
        let trames = (taux as f64 * secondes) as usize;
        let mut v = Vec::with_capacity(trames * canaux as usize);
        for n in 0..trames {
            let t = n as f64 / taux as f64;
            let e = (std::f64::consts::TAU * frequence * t).sin() as f32 * 0.5;
            for _ in 0..canaux {
                v.push(e);
            }
        }
        v
    }

    fn crete(v: &[i16]) -> f32 {
        v.iter().fold(0f32, |a, s| a.max((*s as f32 / 32767.0).abs()))
    }

    /// Frequence estimee par comptage des passages par zero.
    fn frequence(v: &[i16], taux: u32) -> f64 {
        let passages = v
            .windows(2)
            .filter(|p| (p[0] >= 0) != (p[1] >= 0))
            .count() as f64;
        passages * taux as f64 / (2.0 * v.len() as f64)
    }

    #[test]
    fn le_melange_fait_la_moyenne_des_canaux() {
        assert_eq!(melanger_mono(&[1.0, 0.0, 0.5, 0.5], 2), vec![0.5, 0.5]);
    }

    #[test]
    fn deux_canaux_en_opposition_s_annulent() {
        // Preuve que c'est bien un melange et non la reprise d'un seul canal.
        let mono = melanger_mono(&[0.7, -0.7, -0.3, 0.3], 2);
        assert!(mono.iter().all(|v| v.abs() < 1e-6), "{mono:?}");
    }

    #[test]
    fn le_mono_passe_tel_quel() {
        assert_eq!(melanger_mono(&[0.1, 0.2, 0.3], 1), vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn a_16_khz_mono_les_octets_sont_ceux_de_l_entree() {
        // Cas du materiel deja au bon format : aucune transformation, donc aucune perte.
        let mut c = Convertisseur::new(TAUX_CIBLE, 1);
        let octets = c.pousser(&[0.0, 0.5, -0.5, 1.0]);
        assert_eq!(echantillons(&octets), vec![0, 16384, -16384, 32767]);
        assert!(c.fin().is_empty());
    }

    #[test]
    fn un_depassement_est_borne_et_ne_change_pas_de_signe() {
        // `(1.5 * 32767.0) as i16` replierait a -32768 : un pic deviendrait un claquement.
        let mut c = Convertisseur::new(TAUX_CIBLE, 1);
        let octets = c.pousser(&[1.5, -1.5]);
        assert_eq!(echantillons(&octets), vec![32767, -32768]);
    }

    #[test]
    fn les_octets_sont_en_petit_boutien() {
        let mut c = Convertisseur::new(TAUX_CIBLE, 1);
        // 0.5 -> 16384 -> 0x4000 -> 00 40 en petit boutien.
        assert_eq!(c.pousser(&[0.5]), vec![0x00, 0x40]);
    }

    #[test]
    fn de_48_khz_stereo_on_sort_16_khz_mono() {
        let mut c = Convertisseur::new(48_000, 2);
        let mut octets = c.pousser(&sinusoide(1000.0, 48_000, 1.0, 2));
        octets.extend(c.fin());
        let ech = echantillons(&octets);
        // Une seconde a 16 kHz, a quelques echantillons de filtre pres.
        assert!(
            (ech.len() as i64 - 16_000).abs() < 40,
            "{} echantillons",
            ech.len()
        );
        assert!((crete(&ech) - 0.5).abs() < 0.02, "crete {}", crete(&ech));
        let f = frequence(&ech, TAUX_CIBLE);
        assert!((f - 1000.0).abs() < 10.0, "frequence {f}");
    }

    #[test]
    fn un_ton_au_dela_de_nyquist_est_ecrase() {
        // Sans filtre, 10 kHz echantillonne a 16 kHz se replierait a 6 kHz, en pleine
        // bande de parole, et a pleine amplitude.
        let mut c = Convertisseur::new(48_000, 1);
        let mut octets = c.pousser(&sinusoide(10_000.0, 48_000, 1.0, 1));
        octets.extend(c.fin());
        let ech = echantillons(&octets);
        // Bords ecartes : l'attaque et la coupure du signal de test sont des ruptures
        // franches, dont la reponse transitoire du filtre n'a rien a voir avec le
        // repliement qu'on mesure. Mesure du 2026-08-21 : 0 en regime etabli, 0,127 sur
        // les 400 premiers echantillons.
        let niveau = crete(&ech[400..ech.len() - 400]);
        assert!(niveau < 0.005, "repliement a {niveau} (attendu < 0.005)");
    }

    #[test]
    fn un_rapport_non_entier_est_gere() {
        // 44 100 -> 16 000 n'est pas un facteur entier : c'est le cas qu'une decimation
        // naive ne sait pas traiter.
        let mut c = Convertisseur::new(44_100, 1);
        let mut octets = c.pousser(&sinusoide(440.0, 44_100, 1.0, 1));
        octets.extend(c.fin());
        let ech = echantillons(&octets);
        assert!(
            (ech.len() as i64 - 16_000).abs() < 40,
            "{} echantillons",
            ech.len()
        );
        let f = frequence(&ech, TAUX_CIBLE);
        assert!((f - 440.0).abs() < 10.0, "frequence {f}");
    }

    #[test]
    fn decouper_l_entree_ne_change_pas_la_sortie() {
        // Le rappel audio livre des lots de taille quelconque : le resultat ne doit
        // dependre que du signal, pas du decoupage.
        let entree = sinusoide(1000.0, 48_000, 0.3, 2);
        let mut entier = Convertisseur::new(48_000, 2);
        let mut a = entier.pousser(&entree);
        a.extend(entier.fin());

        let mut morceaux = Convertisseur::new(48_000, 2);
        let mut b = Vec::new();
        for lot in entree.chunks(137 * 2) {
            b.extend(morceaux.pousser(lot));
        }
        b.extend(morceaux.fin());

        assert_eq!(a, b);
    }

    #[test]
    fn le_silence_reste_du_silence() {
        // Ce que la detection de piste muette lit : du silence en entree ne doit pas
        // fabriquer de bruit de filtre.
        let mut c = Convertisseur::new(48_000, 2);
        let mut octets = c.pousser(&vec![0.0; 48_000 * 2]);
        octets.extend(c.fin());
        assert!(echantillons(&octets).iter().all(|s| *s == 0));
    }
}
