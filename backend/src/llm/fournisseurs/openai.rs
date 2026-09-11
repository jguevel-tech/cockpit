//! OpenAI : une API de texte et une API de transcription. Pas de CLI, donc rien a reconnaitre
//! dans un terminal.
//!
//! C'est lui qui fait marcher les comptes rendus de reunion depuis le debut. Ce fichier ne
//! garde que ce qui lui est PROPRE — deux adresses, la forme de ses reponses, le nom de ses
//! modeles et sa limite de taille. Le decoupage de l'audio, la detection des pistes muettes et
//! le filtre des hallucinations restent chez nous : voir `llm/texte.rs`.

use crate::llm::texte::{Futur, ModeleTexte, SegmentTranscrit, Transcription};
use crate::llm::Fournisseur;
use serde::Deserialize;
use serde_json::json;

pub static OPENAI: OpenAi = OpenAi;
static TEXTE: TexteOpenAi = TexteOpenAi;
static TRANSCRIPTION: TranscriptionOpenAi = TranscriptionOpenAi;

/// La limite de taille d'un envoi audio, annoncee par l'API : 25 Mo. C'est elle qui decide du
/// decoupage, et elle appartient au fournisseur — la fixer chez nous ferait echouer le premier
/// fournisseur plus strict avec « HTTP 413 » pour toute explication.
const TAILLE_MAXIMALE: usize = 25 * 1024 * 1024;

pub struct OpenAi;

impl Fournisseur for OpenAi {
    fn id(&self) -> &'static str {
        "openai"
    }
    fn nom(&self) -> &'static str {
        "OpenAI"
    }
    fn symbole(&self) -> &'static str {
        "◍"
    }
    fn couleur(&self) -> &'static str {
        "#10a37f"
    }
    fn texte(&self) -> Option<&'static dyn ModeleTexte> {
        Some(&TEXTE)
    }
    fn transcription(&self) -> Option<&'static dyn Transcription> {
        Some(&TRANSCRIPTION)
    }
    fn cle_requise(&self) -> bool {
        true
    }
}

// ---------- Texte ----------

pub struct TexteOpenAi;

#[derive(Deserialize)]
struct ReponseChat {
    choices: Vec<Choix>,
}

#[derive(Deserialize)]
struct Choix {
    message: MessageChat,
}

#[derive(Deserialize)]
struct MessageChat {
    content: String,
}

impl ModeleTexte for TexteOpenAi {
    fn modele_par_defaut(&self) -> &'static str {
        "gpt-4o"
    }

    fn repondre<'a>(
        &'a self,
        client: &'a reqwest::Client,
        cle: &'a str,
        modele: &'a str,
        consigne: &'a str,
        contenu: &'a str,
    ) -> Futur<'a, String> {
        Box::pin(async move {
            let corps = json!({
                "model": modele,
                "messages": [
                    { "role": "system", "content": consigne },
                    { "role": "user", "content": contenu },
                ],
            });

            let reponse = client
                .post("https://api.openai.com/v1/chat/completions")
                .bearer_auth(cle)
                .json(&corps)
                .send()
                .await
                .map_err(|e| format!("appel API resume: {e}"))?;

            // LE CODE DE RETOUR EST LU : une erreur rendue comme un corps vide fabriquerait un
            // compte rendu vide, presente comme normal.
            let statut = reponse.status();
            let texte = reponse.text().await.map_err(|e| e.to_string())?;
            if !statut.is_success() {
                let court: String = texte.chars().take(300).collect();
                return Err(format!("API resume HTTP {statut}: {court}"));
            }

            let analysee: ReponseChat = serde_json::from_str(&texte)
                .map_err(|e| format!("reponse resume invalide: {e}"))?;
            analysee
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .ok_or_else(|| "reponse resume vide".to_string())
        })
    }
}

// ---------- Transcription ----------

pub struct TranscriptionOpenAi;

#[derive(Deserialize)]
struct ReponseTranscription {
    segments: Option<Vec<SegmentApi>>,
}

#[derive(Deserialize)]
struct SegmentApi {
    start: f64,
    text: String,
    #[serde(default)]
    no_speech_prob: f64,
}

impl Transcription for TranscriptionOpenAi {
    fn taille_maximale(&self) -> usize {
        TAILLE_MAXIMALE
    }

    fn transcrire<'a>(
        &'a self,
        client: &'a reqwest::Client,
        cle: &'a str,
        wav: Vec<u8>,
        langue: &'a str,
    ) -> Futur<'a, Vec<SegmentTranscrit>> {
        Box::pin(async move {
            let morceau = reqwest::multipart::Part::bytes(wav)
                .file_name("morceau.wav")
                .mime_str("audio/wav")
                .map_err(|e| e.to_string())?;
            let formulaire = reqwest::multipart::Form::new()
                .part("file", morceau)
                .text("model", "whisper-1")
                // `verbose_json` et non `json` : c'est le seul format qui rend les horodatages
                // et la probabilite de non-parole, dont depend le filtre d'hallucinations.
                .text("response_format", "verbose_json")
                .text("language", langue.to_string());

            let reponse = client
                .post("https://api.openai.com/v1/audio/transcriptions")
                .bearer_auth(cle)
                .multipart(formulaire)
                .send()
                .await
                .map_err(|e| format!("appel API transcription: {e}"))?;

            let statut = reponse.status();
            let corps = reponse.text().await.map_err(|e| e.to_string())?;
            if !statut.is_success() {
                let court: String = corps.chars().take(300).collect();
                return Err(format!("API transcription HTTP {statut}: {court}"));
            }

            let analysee: ReponseTranscription = serde_json::from_str(&corps)
                .map_err(|e| format!("reponse transcription invalide: {e}"))?;
            Ok(analysee
                .segments
                .unwrap_or_default()
                .into_iter()
                .map(|s| SegmentTranscrit {
                    debut: s.start,
                    texte: s.text,
                    non_parole: s.no_speech_prob,
                })
                .collect())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::llm::Fournisseur;

    /// OpenAI n'a pas de CLI : il ne doit rien apporter a la detection d'un agent dans un
    /// terminal, sinon `ollama` ou un dossier nomme « openai » declencherait le repere.
    #[test]
    fn openai_n_a_pas_de_cli_mais_deux_capacites() {
        let openai = &super::OPENAI;
        assert!(openai.commandes().is_empty());
        assert!(openai.texte().is_some());
        assert!(openai.transcription().is_some());
        assert!(openai.cle_requise(), "sans cle il ne sert a rien");
        assert!(openai.conversations().is_none());
        assert!(openai.abonnement().is_none());
    }

    #[test]
    fn le_modele_par_defaut_appartient_au_fournisseur() {
        let modele = super::OPENAI.texte().unwrap().modele_par_defaut();
        assert_eq!(modele, "gpt-4o");
    }
}
