//! Transcription via l'API OpenAI (whisper-1) et fusion des deux pistes en dialogue.

use super::wav;
use serde::Deserialize;
use std::path::Path;

/// Duree d'un chunk envoye a l'API : 600 s = ~19,2 Mo de WAV, sous la limite de 25 Mo.
const CHUNK_SECS: usize = 600;
/// En dessous de cette amplitude max (sur 32767), le chunk est considere silencieux.
const SILENCE_AMPLITUDE: i32 = 500;
/// Segments Whisper avec une proba de non-parole au-dela = hallucination probable.
const NO_SPEECH_MAX: f64 = 0.75;

/// Phrases que Whisper hallucine sur du silence ou de la musique
/// (heritees de son corpus d'entrainement YouTube).
const HALLUCINATION_MARKERS: &[&str] = &[
    "sous-titres réalisés",
    "sous-titrage",
    "amara.org",
    "merci d'avoir regardé",
    "voir une autre vidéo",
    "abonnez-vous",
    "n'hésitez pas à vous abonner",
    "à bientôt pour une nouvelle vidéo",
];

fn is_hallucination(text: &str) -> bool {
    let lower = text.to_lowercase();
    HALLUCINATION_MARKERS.iter().any(|m| lower.contains(m))
}

#[derive(Debug, Clone)]
pub struct Segment {
    pub start: f64,
    pub text: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    segments: Option<Vec<ApiSegment>>,
}

#[derive(Deserialize)]
struct ApiSegment {
    start: f64,
    text: String,
    #[serde(default)]
    no_speech_prob: f64,
}

async fn transcribe_chunk(
    client: &reqwest::Client,
    api_key: &str,
    wav_bytes: Vec<u8>,
) -> Result<Vec<ApiSegment>, String> {
    let part = reqwest::multipart::Part::bytes(wav_bytes)
        .file_name("chunk.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", "whisper-1")
        .text("response_format", "verbose_json")
        .text("language", "fr");

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("appel API transcription: {}", e))?;

    let status = resp.status();
    let body = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("API transcription HTTP {}: {}", status, truncate(&body, 300)));
    }

    let parsed: ApiResponse =
        serde_json::from_str(&body).map_err(|e| format!("reponse transcription invalide: {}", e))?;
    Ok(parsed.segments.unwrap_or_default())
}

/// Ce qu'on sait d'une piste avant de la transcrire.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EtatPiste {
    /// Fichier absent ou vide : la piste n'a jamais demarre (deja dit au demarrage).
    Absente,
    /// Des octets, mais tous nuls : la piste n'a jamais recu un seul echantillon utile.
    Muette,
    Sonore,
}

/// Une piste ENTIEREMENT a zero n'est pas une piste calme.
///
/// C'est le symptome d'un tap macOS sans autorisation TCC : le flux tourne, les rappels
/// arrivent, tout est nul, et aucune erreur n'est levee. Sans cette distinction, le filtre
/// de silence sautait tous les chunks et le pipeline finissait sur « Aucune parole
/// detectee » — un message qui envoie chercher au mauvais endroit.
///
/// Lecture en flot avec sortie au premier octet non nul : sur une piste normale, ca
/// s'arrete au premier bloc.
pub fn etat_piste(raw_path: &Path) -> EtatPiste {
    use std::io::Read;
    let Ok(fichier) = std::fs::File::open(raw_path) else {
        return EtatPiste::Absente;
    };
    let mut lecteur = std::io::BufReader::new(fichier);
    let mut tampon = [0u8; 64 * 1024];
    let mut vu = false;
    while let Ok(lu) = lecteur.read(&mut tampon) {
        if lu == 0 {
            break;
        }
        vu = true;
        if tampon[..lu].iter().any(|o| *o != 0) {
            return EtatPiste::Sonore;
        }
    }
    if vu {
        EtatPiste::Muette
    } else {
        EtatPiste::Absente
    }
}

/// Message d'echec quand tout ce qui a ete capte est a zero strict.
///
/// Il NOMME la cause au lieu de parler de parole non detectee : sur macOS c'est une
/// autorisation qui manque, et l'utilisateur n'a aucun moyen de le deviner.
pub fn message_silence_total() -> String {
    let mut message = "Aucun son n'a ete capte : les pistes audio sont entierement vides \
         (pas un seul echantillon non nul)."
        .to_string();
    if cfg!(target_os = "macos") {
        message.push_str(
            " Sur macOS, la capture audio passe par une autorisation liee a la signature \
             de l'application : une application non signee obtient des pistes muettes, \
             sans erreur.",
        );
    }
    message
}

/// Transcrit une piste PCM brute complete : decoupe en chunks, saute les silences,
/// filtre les hallucinations, decale les timestamps par chunk.
pub async fn transcribe_track(
    client: &reqwest::Client,
    api_key: &str,
    raw_path: &Path,
) -> Result<Vec<Segment>, String> {
    let pcm = std::fs::read(raw_path)
        .map_err(|e| format!("lecture {}: {}", raw_path.display(), e))?;

    let chunk_bytes = CHUNK_SECS * wav::BYTES_PER_SEC;
    let mut segments = Vec::new();

    for (idx, chunk) in pcm.chunks(chunk_bytes).enumerate() {
        // Moins d'une demi-seconde d'audio ou quasi-silence : on saute
        if chunk.len() < wav::BYTES_PER_SEC / 2 || wav::max_amplitude(chunk) < SILENCE_AMPLITUDE {
            continue;
        }
        let offset = (idx * CHUNK_SECS) as f64;
        let api_segments = transcribe_chunk(client, api_key, wav::wav_from_pcm(chunk)).await?;
        for s in api_segments {
            let text = s.text.trim();
            if text.is_empty() || s.no_speech_prob > NO_SPEECH_MAX || is_hallucination(text) {
                continue;
            }
            segments.push(Segment {
                start: s.start + offset,
                text: text.to_string(),
            });
        }
    }

    Ok(segments)
}

/// Fusionne les segments des deux pistes en un dialogue chronologique Markdown.
/// Les segments consecutifs d'un meme locuteur sont regroupes en un paragraphe.
pub fn merge_dialogue(mic: Vec<Segment>, system: Vec<Segment>) -> String {
    let mut all: Vec<(&'static str, Segment)> = mic
        .into_iter()
        .map(|s| ("Moi", s))
        .chain(system.into_iter().map(|s| ("Eux", s)))
        .collect();
    all.sort_by(|a, b| a.1.start.partial_cmp(&b.1.start).unwrap_or(std::cmp::Ordering::Equal));

    let mut out = String::new();
    let mut current: Option<(&str, f64, Vec<String>)> = None;

    for (speaker, seg) in all {
        match &mut current {
            Some((cur_speaker, _, texts)) if *cur_speaker == speaker => {
                texts.push(seg.text);
            }
            _ => {
                if let Some((sp, start, texts)) = current.take() {
                    push_line(&mut out, sp, start, &texts);
                }
                current = Some((speaker, seg.start, vec![seg.text]));
            }
        }
    }
    if let Some((sp, start, texts)) = current {
        push_line(&mut out, sp, start, &texts);
    }
    out
}

fn push_line(out: &mut String, speaker: &str, start: f64, texts: &[String]) {
    out.push_str(&format!(
        "**{}** [{}] : {}\n\n",
        speaker,
        format_timestamp(start),
        texts.join(" ")
    ));
}

pub fn format_timestamp(secs: f64) -> String {
    let total = secs.max(0.0) as u64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f64, text: &str) -> Segment {
        Segment { start, text: text.into() }
    }

    #[test]
    fn test_is_hallucination() {
        assert!(is_hallucination("Sous-titres réalisés para la communauté d'Amara.org"));
        assert!(is_hallucination("Merci d'avoir regardé cette vidéo !"));
        assert!(is_hallucination("Voir une autre vidéo ..."));
        assert!(!is_hallucination("On regarde la vidéo du client ensemble demain"));
    }

    #[test]
    fn une_piste_de_zeros_est_muette_et_non_absente() {
        let dir = std::env::temp_dir().join(format!("cockpit-etat-piste-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let muette = dir.join("muette.raw");
        std::fs::write(&muette, vec![0u8; 32_000]).unwrap();
        assert_eq!(etat_piste(&muette), EtatPiste::Muette);

        let sonore = dir.join("sonore.raw");
        let mut octets = vec![0u8; 32_000];
        // Un seul echantillon non nul, tout a la fin : la lecture doit aller jusqu'au bout
        // avant de conclure au silence.
        octets[31_999] = 3;
        std::fs::write(&sonore, octets).unwrap();
        assert_eq!(etat_piste(&sonore), EtatPiste::Sonore);

        let vide = dir.join("vide.raw");
        std::fs::write(&vide, Vec::<u8>::new()).unwrap();
        assert_eq!(etat_piste(&vide), EtatPiste::Absente);
        assert_eq!(etat_piste(&dir.join("jamais-creee.raw")), EtatPiste::Absente);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn le_message_de_silence_total_ne_parle_pas_de_parole_non_detectee() {
        let m = message_silence_total();
        assert!(m.contains("entierement vides"), "{m}");
        assert!(!m.to_lowercase().contains("parole"), "{m}");
    }

    #[test]
    fn test_format_timestamp() {
        assert_eq!(format_timestamp(0.0), "00:00");
        assert_eq!(format_timestamp(75.4), "01:15");
        assert_eq!(format_timestamp(3725.0), "1:02:05");
    }

    #[test]
    fn test_merge_dialogue_groups_consecutive_speakers() {
        let mic = vec![seg(0.0, "Bonjour."), seg(3.0, "Ca va ?")];
        let system = vec![seg(6.0, "Oui et toi ?")];
        let out = merge_dialogue(mic, system);
        assert_eq!(
            out,
            "**Moi** [00:00] : Bonjour. Ca va ?\n\n**Eux** [00:06] : Oui et toi ?\n\n"
        );
    }

    #[test]
    fn test_merge_dialogue_interleaves_by_time() {
        let mic = vec![seg(5.0, "Reponse.")];
        let system = vec![seg(1.0, "Question ?")];
        let out = merge_dialogue(mic, system);
        assert!(out.starts_with("**Eux** [00:01]"));
        assert!(out.contains("**Moi** [00:05]"));
    }
}
