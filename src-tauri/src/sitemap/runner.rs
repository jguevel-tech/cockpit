use super::{diff::compute_diff, fetcher, parser::parse_sitemap_xml};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

const CONCURRENCY: usize = 5;
const DIFF_MAX_LINES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingItem {
    pub url: String,
    pub status_code: Option<u16>,
    pub ok: bool,
    pub error: Option<String>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct PingReport {
    pub pair_id: i64,
    pub total: usize,
    pub ok: usize,
    pub ko: usize,
    pub items: Vec<PingItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DiffStatus {
    Equal,
    Different,
    OrphanRef,
    OrphanCheck,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffItem {
    pub path: String,
    pub ref_url: Option<String>,
    pub check_url: Option<String>,
    pub status: DiffStatus,
    pub ref_bytes: Option<usize>,
    pub check_bytes: Option<usize>,
    pub diff: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffReport {
    pub pair_id: i64,
    pub total: usize,
    pub equal: usize,
    pub different: usize,
    pub orphans: usize,
    pub errors: usize,
    pub items: Vec<DiffItem>,
}

#[derive(Serialize, Clone)]
struct ProgressPayload<'a> {
    pair_id: i64,
    mode: &'a str,
    done: usize,
    total: usize,
    current_url: &'a str,
    status: &'a str,
    detail: &'a str,
}

fn emit_progress(
    app: &AppHandle,
    pair_id: i64,
    mode: &str,
    done: usize,
    total: usize,
    current_url: &str,
    status: &str,
    detail: &str,
) {
    let _ = app.emit(
        "sitemap_check_progress",
        ProgressPayload {
            pair_id,
            mode,
            done,
            total,
            current_url,
            status,
            detail,
        },
    );
}

pub async fn run_ping(
    app: AppHandle,
    pair_id: i64,
    sitemap_url: &str,
    query_suffix: &str,
    skip_urls: Vec<String>,
    limit: Option<usize>,
    cancel: Arc<AtomicBool>,
) -> Result<PingReport, String> {
    let client = fetcher::build_client()?;
    let skip: HashSet<String> = skip_urls.into_iter().collect();

    // 1. Fetch the sitemap
    let (status, body) = fetcher::fetch_text(&client, sitemap_url).await?;
    if !(200..300).contains(&status) {
        return Err(format!("sitemap fetch returned HTTP {}", status));
    }

    // 2. Parse URLs, apply limit, filter out already-processed ones
    let mut all_urls = parse_sitemap_xml(&body)?;
    if let Some(n) = limit {
        all_urls.truncate(n);
    }
    let urls: Vec<String> = all_urls
        .into_iter()
        .map(|u| fetcher::append_query(&u, query_suffix))
        .filter(|u| !skip.contains(u))
        .collect();
    let total = urls.len();
    if total == 0 {
        return Ok(PingReport {
            pair_id,
            total: 0,
            ok: 0,
            ko: 0,
            items: vec![],
        });
    }

    // 3. Ping each in parallel (cancellation-aware)
    let client_ref = &client;
    let app_ref = app.clone();
    let cancel_ref = cancel.clone();
    let results: Vec<PingItem> = stream::iter(urls.into_iter().enumerate())
        .map(|(idx, full_url)| {
            let app = app_ref.clone();
            let cancel = cancel_ref.clone();
            async move {
                if cancel.load(Ordering::SeqCst) {
                    let item = PingItem {
                        url: full_url.clone(),
                        status_code: None,
                        ok: false,
                        error: Some("annule".into()),
                        duration_ms: 0,
                    };
                    emit_progress(&app, pair_id, "ping", idx + 1, total, &full_url, "cancelled", "annule");
                    return item;
                }
                let start = std::time::Instant::now();
                let (status_code, ok, error) = match fetcher::fetch_status(client_ref, &full_url).await {
                    Ok(code) => (Some(code), (200..400).contains(&code), None),
                    Err(e) => (None, false, Some(e)),
                };
                let duration_ms = start.elapsed().as_millis();
                let (ev_status, ev_detail) = if ok {
                    ("ok", format!("HTTP {} ({} ms)", status_code.unwrap_or(0), duration_ms))
                } else if let Some(code) = status_code {
                    ("ko", format!("HTTP {} ({} ms)", code, duration_ms))
                } else {
                    ("error", error.clone().unwrap_or_else(|| "erreur".into()))
                };
                let item = PingItem {
                    url: full_url.clone(),
                    status_code,
                    ok,
                    error,
                    duration_ms,
                };
                emit_progress(&app, pair_id, "ping", idx + 1, total, &full_url, ev_status, &ev_detail);
                item
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    let ok = results.iter().filter(|i| i.ok).count();
    let ko = total - ok;
    Ok(PingReport {
        pair_id,
        total,
        ok,
        ko,
        items: results,
    })
}

pub async fn run_diff(
    app: AppHandle,
    pair_id: i64,
    sitemap_ref_url: &str,
    sitemap_check_url: &str,
    ref_query: &str,
    check_query: &str,
    skip_paths: Vec<String>,
    limit: Option<usize>,
    cancel: Arc<AtomicBool>,
) -> Result<DiffReport, String> {
    let client = fetcher::build_client()?;
    let skip: HashSet<String> = skip_paths.into_iter().collect();

    // 1. Fetch both sitemaps (in parallel)
    let (ref_result, check_result) = futures::join!(
        fetcher::fetch_text(&client, sitemap_ref_url),
        fetcher::fetch_text(&client, sitemap_check_url)
    );
    let (ref_status, ref_body) = ref_result.map_err(|e| format!("ref sitemap: {}", e))?;
    let (check_status, check_body) = check_result.map_err(|e| format!("check sitemap: {}", e))?;
    if !(200..300).contains(&ref_status) {
        return Err(format!("ref sitemap HTTP {}", ref_status));
    }
    if !(200..300).contains(&check_status) {
        return Err(format!("check sitemap HTTP {}", check_status));
    }

    // 2. Parse both
    let ref_urls = parse_sitemap_xml(&ref_body)?;
    let check_urls = parse_sitemap_xml(&check_body)?;

    // 3. Index by path+query (origin-agnostic)
    let mut ref_map: HashMap<String, String> = ref_urls
        .iter()
        .map(|u| (fetcher::url_path_and_query(u), u.clone()))
        .collect();
    let mut check_map: HashMap<String, String> = check_urls
        .iter()
        .map(|u| (fetcher::url_path_and_query(u), u.clone()))
        .collect();

    let mut all_paths: Vec<String> = ref_map.keys().cloned().collect();
    for k in check_map.keys() {
        if !ref_map.contains_key(k) {
            all_paths.push(k.clone());
        }
    }
    all_paths.sort();
    // Apply limit BEFORE skip, so resume respects the same bounded set.
    if let Some(n) = limit {
        all_paths.truncate(n);
    }
    all_paths.retain(|k| !skip.contains(k));

    let total = all_paths.len();
    let client_ref = &client;
    let app_ref = app.clone();
    let cancel_ref = cancel.clone();

    // 4. For each path: fetch both sides, compare
    let results: Vec<DiffItem> = stream::iter(all_paths.into_iter().enumerate())
        .map(|(idx, path)| {
            let ref_url = ref_map.remove(&path);
            let check_url = check_map.remove(&path);
            let app = app_ref.clone();
            let cancel = cancel_ref.clone();
            let ref_q = ref_query.to_string();
            let check_q = check_query.to_string();
            async move {
                if cancel.load(Ordering::SeqCst) {
                    emit_progress(&app, pair_id, "diff", idx + 1, total, &path, "cancelled", "annule");
                    return DiffItem {
                        path: path.clone(),
                        ref_url,
                        check_url,
                        status: DiffStatus::Error,
                        ref_bytes: None,
                        check_bytes: None,
                        diff: None,
                        error: Some("annule".into()),
                    };
                }
                let item = match (&ref_url, &check_url) {
                    (Some(r), Some(c)) => {
                        let r_full = fetcher::append_query(r, &ref_q);
                        let c_full = fetcher::append_query(c, &check_q);
                        let (r_res, c_res) = futures::join!(
                            fetcher::fetch_text(client_ref, &r_full),
                            fetcher::fetch_text(client_ref, &c_full)
                        );
                        match (r_res, c_res) {
                            (Ok((_, rb)), Ok((_, cb))) => {
                                let diff = compute_diff(&rb, &cb, DIFF_MAX_LINES);
                                let status = if diff.is_none() {
                                    DiffStatus::Equal
                                } else {
                                    DiffStatus::Different
                                };
                                DiffItem {
                                    path: path.clone(),
                                    ref_url: Some(r_full),
                                    check_url: Some(c_full),
                                    status,
                                    ref_bytes: Some(rb.len()),
                                    check_bytes: Some(cb.len()),
                                    diff,
                                    error: None,
                                }
                            }
                            (Err(e), _) | (_, Err(e)) => DiffItem {
                                path: path.clone(),
                                ref_url: Some(r_full),
                                check_url: Some(c_full),
                                status: DiffStatus::Error,
                                ref_bytes: None,
                                check_bytes: None,
                                diff: None,
                                error: Some(e),
                            },
                        }
                    }
                    (Some(r), None) => DiffItem {
                        path: path.clone(),
                        ref_url: Some(r.clone()),
                        check_url: None,
                        status: DiffStatus::OrphanRef,
                        ref_bytes: None,
                        check_bytes: None,
                        diff: None,
                        error: None,
                    },
                    (None, Some(c)) => DiffItem {
                        path: path.clone(),
                        ref_url: None,
                        check_url: Some(c.clone()),
                        status: DiffStatus::OrphanCheck,
                        ref_bytes: None,
                        check_bytes: None,
                        diff: None,
                        error: None,
                    },
                    (None, None) => unreachable!(),
                };
                let (ev_status, ev_detail) = match &item.status {
                    DiffStatus::Equal => (
                        "equal",
                        format!("{} o identiques", item.ref_bytes.unwrap_or(0)),
                    ),
                    DiffStatus::Different => (
                        "different",
                        format!(
                            "ref {} o / check {} o",
                            item.ref_bytes.unwrap_or(0),
                            item.check_bytes.unwrap_or(0)
                        ),
                    ),
                    DiffStatus::OrphanRef => ("orphan_ref", "present cote ref uniquement".to_string()),
                    DiffStatus::OrphanCheck => ("orphan_check", "present cote check uniquement".to_string()),
                    DiffStatus::Error => (
                        "error",
                        item.error.clone().unwrap_or_else(|| "erreur".into()),
                    ),
                };
                emit_progress(&app, pair_id, "diff", idx + 1, total, &path, ev_status, &ev_detail);
                item
            }
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    let equal = results.iter().filter(|i| i.status == DiffStatus::Equal).count();
    let different = results.iter().filter(|i| i.status == DiffStatus::Different).count();
    let orphans = results
        .iter()
        .filter(|i| matches!(i.status, DiffStatus::OrphanRef | DiffStatus::OrphanCheck))
        .count();
    let errors = results.iter().filter(|i| i.status == DiffStatus::Error).count();

    Ok(DiffReport {
        pair_id,
        total,
        equal,
        different,
        orphans,
        errors,
        items: results,
    })
}
