use std::time::Duration;

pub const USER_AGENT: &str = "cockpit-sitemap-check/0.1";
pub const TIMEOUT_SECS: u64 = 30;

pub fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("build http client: {}", e))
}

/// Ajoute une query au format "?a=b" ou "&a=b" a une URL existante.
/// Gere le cas ou l'URL a deja une query (append avec &) ou non (remplace ? par ?).
pub fn append_query(url: &str, extra: &str) -> String {
    let extra = extra.trim();
    if extra.is_empty() {
        return url.to_string();
    }
    // Retirer un leading ? ou & pour simplifier
    let extra = extra.trim_start_matches(['?', '&']);
    if extra.is_empty() {
        return url.to_string();
    }
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{}{}{}", url, sep, extra)
}

/// Extrait path + query d'une URL (sans scheme+host) pour matcher entre deux domaines.
/// Exemple: "https://prod.com/foo?x=1" -> "/foo?x=1".
pub fn url_path_and_query(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(parsed) => {
            let mut out = parsed.path().to_string();
            if let Some(q) = parsed.query() {
                out.push('?');
                out.push_str(q);
            }
            out
        }
        Err(_) => url.to_string(),
    }
}

pub async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<(u16, String), String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request: {}", e))?;
    let status = resp.status().as_u16();
    let body = resp.text().await.map_err(|e| format!("body: {}", e))?;
    Ok((status, body))
}

pub async fn fetch_status(client: &reqwest::Client, url: &str) -> Result<u16, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("request: {}", e))?;
    Ok(resp.status().as_u16())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_query_empty() {
        assert_eq!(append_query("https://a.com/p", ""), "https://a.com/p");
    }

    #[test]
    fn test_append_query_no_existing() {
        assert_eq!(
            append_query("https://a.com/p", "new=1"),
            "https://a.com/p?new=1"
        );
    }

    #[test]
    fn test_append_query_with_leading_question_mark() {
        assert_eq!(
            append_query("https://a.com/p", "?new=1"),
            "https://a.com/p?new=1"
        );
    }

    #[test]
    fn test_append_query_with_existing_query() {
        assert_eq!(
            append_query("https://a.com/p?x=1", "new=1"),
            "https://a.com/p?x=1&new=1"
        );
    }

    #[test]
    fn test_append_query_with_existing_and_leading_amp() {
        assert_eq!(
            append_query("https://a.com/p?x=1", "&new=1"),
            "https://a.com/p?x=1&new=1"
        );
    }

    #[test]
    fn test_url_path_and_query_basic() {
        assert_eq!(
            url_path_and_query("https://prod.com/foo/bar?x=1"),
            "/foo/bar?x=1"
        );
    }

    #[test]
    fn test_url_path_and_query_no_query() {
        assert_eq!(url_path_and_query("https://prod.com/foo"), "/foo");
    }

    #[test]
    fn test_url_path_and_query_invalid_returns_input() {
        let bad = "not a url";
        assert_eq!(url_path_and_query(bad), bad);
    }
}
