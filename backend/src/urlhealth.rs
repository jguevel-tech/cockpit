//! Statut up/down des liens rapides d'un projet : un HEAD HTTP (repli GET si le
//! serveur refuse HEAD), 5 s de timeout, redirections suivies. 2xx/3xx = up.

use serde::Serialize;
use std::time::Duration;

#[derive(Serialize, Clone)]
pub struct UrlHealth {
    pub ok: bool,
    /// Code HTTP final (0 si la requete n'a pas abouti).
    pub status: u16,
    pub error: String,
}

fn down(error: String) -> UrlHealth {
    UrlHealth { ok: false, status: 0, error }
}

pub async fn check_url(client: &reqwest::Client, url: &str) -> UrlHealth {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return down("seuls http(s) sont vérifiables".into());
    }

    let head = client.head(url).send().await;
    let resp = match head {
        // Certains serveurs refusent HEAD : on retente en GET avant de conclure
        Ok(r) if r.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
            || r.status() == reqwest::StatusCode::NOT_IMPLEMENTED =>
        {
            client.get(url).send().await
        }
        other => other,
    };

    match resp {
        Ok(r) => {
            let status = r.status();
            UrlHealth {
                ok: status.is_success() || status.is_redirection(),
                status: status.as_u16(),
                error: if status.is_success() || status.is_redirection() {
                    String::new()
                } else {
                    format!("HTTP {}", status.as_u16())
                },
            }
        }
        Err(e) => {
            // Message court et utile ("connexion refusée", "délai dépassé"), pas la chaine
            // de causes complete de reqwest
            let msg = if e.is_timeout() {
                "délai dépassé".to_string()
            } else if e.is_connect() {
                "connexion impossible".to_string()
            } else {
                e.to_string().chars().take(120).collect()
            };
            down(msg)
        }
    }
}

/// Verifie un lot d'URLs en parallele (les liens d'un projet, quelques unites).
pub async fn check_urls(urls: &[String]) -> Vec<UrlHealth> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => return urls.iter().map(|_| down(e.to_string())).collect(),
    };
    futures::future::join_all(urls.iter().map(|u| check_url(&client, u))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scheme_non_http_refuse_sans_requete() {
        let res = check_urls(&["ftp://exemple.org".into(), "pas-une-url".into()]).await;
        assert_eq!(res.len(), 2);
        assert!(!res[0].ok);
        assert!(!res[1].ok);
        assert_eq!(res[0].status, 0);
    }
}
