//! Generation du resume via l'API OpenAI (chat completions).

use serde::Deserialize;
use serde_json::json;

pub const DEFAULT_MODEL: &str = "gpt-4o";

pub const DEFAULT_PROMPT: &str = "\
Tu recois la transcription d'une reunion sous forme de dialogue entre \"Moi\" (l'utilisateur, \
pour qui ce compte rendu est redige) et \"Eux\" (les autres participants). Des prenoms peuvent \
apparaitre dans les propos : utilise-les quand tu peux identifier qui parle ou qui est concerne. \
Si \"Moi\" se presente par son prenom pendant la reunion, ce prenom et \"Moi\" designent la meme \
personne : ne les traite jamais comme deux participants distincts.

Redige en francais un compte rendu DETAILLE et structure en Markdown. Ne cherche pas a etre \
bref : vise l'exhaustivite (l'equivalent d'une a deux pages pour une heure de reunion). \
Conserve les details techniques, les chiffres, les noms d'outils, les exemples concrets, \
les limites et pieges evoques.

Structure attendue :

## Contexte
Sujet de la reunion, participants identifiables, objectif.

## Deroule detaille
Un sous-titre (###) par sujet aborde, dans l'ordre de la reunion. Pour chaque sujet : \
les explications donnees, les exemples, les chiffres, les questions posees et les reponses \
apportees.

## Decisions
Tout ce qui a ete acte, meme informellement.

## Demandes et evolutions evoquees
TOUTES les demandes de fonctionnalites, developpements, corrections de bugs ou ameliorations \
mentionnees au fil de la reunion, meme en passant, avec qui les a demandees. Ne rien omettre.

## Actions
Liste exhaustive de qui doit faire quoi, avec echeance si mentionnee. Commence par les actions \
de \"Moi\" : relis toute la transcription et recense TOUT ce que \"Moi\" s'est engage a faire \
(developpements, envois de mails, recaps, acces, corrections de bugs promis).

Sois factuel : n'invente rien qui ne soit pas dans la transcription. Ignore uniquement les \
banalites (salutations, logistique de connexion, problemes de micro).";

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

pub async fn summarize(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system_prompt: &str,
    transcript: &str,
) -> Result<String, String> {
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": transcript },
        ],
    });

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("appel API resume: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        let short: String = text.chars().take(300).collect();
        return Err(format!("API resume HTTP {}: {}", status, short));
    }

    let parsed: ChatResponse =
        serde_json::from_str(&text).map_err(|e| format!("reponse resume invalide: {}", e))?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "reponse resume vide".to_string())
}
