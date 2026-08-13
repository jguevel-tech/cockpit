# Sitemap Check — Design

**Date** : 2026-04-23
**Statut** : Approuvé (user : "go coder")

## But

Valider qu'un refactor ne casse rien en comparant le HTML servi avant/après, URL par URL, à partir d'un sitemap. Deux modes : **ping** (statuts HTTP d'une seule source) et **diff** (comparaison d'une paire référence/à-vérifier).

## Décisions (validées en brainstorming)

- **Modèle de comparaison** : A — live vs live (deux URLs de sitemap fetchées en temps réel, pas de snapshot figé).
- **Organisation** : paires (ref, check) scopées par projet. Les deux URLs de sitemap peuvent être identiques ; dans ce cas, la différenciation se fait via un query param ajouté côté check (le code en prod sait interpréter un flag pour servir l'ancien ou le nouveau comportement).
- **Diff** : brut (pas de normalisation). Assumé : deux URLs prod du même sitemap doivent rendre un HTML strictement identique.
- **Format sitemap** : XML standard (`<urlset><url><loc>...</loc></url></urlset>`). Pas de support sitemapindex en V1.
- **UI** : nouvel onglet "Sitemap" par projet. Pas d'historique persisté — seule la dernière run est en mémoire côté frontend.
- **Implémentation** : Rust natif (reqwest + quick-xml + similar), pas de shell out vers curl.

## Data model

Table unique `sitemap_pairs`, même pattern que `urls` :

```sql
CREATE TABLE sitemap_pairs (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    project           TEXT NOT NULL,
    label             TEXT NOT NULL,
    sitemap_ref_url   TEXT NOT NULL,
    sitemap_check_url TEXT NOT NULL,
    ref_query         TEXT NOT NULL DEFAULT '',
    check_query       TEXT NOT NULL DEFAULT '',
    position          INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_sitemap_pairs_project ON sitemap_pairs(project);
```

`ref_query` et `check_query` : suffixe ajouté à chaque URL fetchée (ex: `?new=1`). Fusion propre avec la query existante (si l'URL a déjà des params, on append avec `&`).

**Matching des URLs entre les deux sitemaps** : par `path + query` de l'URL (ignore origine). Les URLs présentes que d'un côté sont reportées comme "orphelines".

## IPC (commandes Tauri)

CRUD :
- `get_sitemap_pairs(project) -> Vec<SitemapPair>`
- `create_sitemap_pair(project, label, sitemap_ref_url, sitemap_check_url, ref_query, check_query) -> SitemapPair`
- `update_sitemap_pair(id, label, sitemap_ref_url, sitemap_check_url, ref_query, check_query) -> SitemapPair`
- `delete_sitemap_pair(id)`

Actions :
- `run_sitemap_ping(pair_id) -> PingReport` : fetch `sitemap_ref_url`, parse, HEAD/GET chaque URL avec `ref_query`, retourne statuts (OK/KO + code HTTP + durée).
- `run_sitemap_diff(pair_id) -> DiffReport` : fetch les deux sitemaps, match les URLs, GET chaque paire, compare les bytes, retourne diff par URL (égalité, taille, aperçu unifié si différent).

Events streamés pendant les runs :
- `sitemap_check_progress` : `{ pair_id, mode, done, total, current_url, status }`.

## Concurrence & timeouts

- **Parallélisme** : 5 fetchs simultanés (`futures::stream::buffer_unordered(5)`).
- **Timeout** : 30 s par requête HTTP.
- **User-Agent** : `cockpit-sitemap-check/0.1`.

## Module Rust

Nouveau module `src-tauri/src/sitemap/` :
- `mod.rs` : réexport
- `parser.rs` : parse sitemap XML → `Vec<String>` d'URLs
- `fetcher.rs` : client reqwest partagé, fetch HTML avec timeout
- `diff.rs` : comparaison brute (bytes) + diff unifié via `similar`
- `runner.rs` : orchestration ping + diff, émission d'events

Stockage : `src-tauri/src/storage/sitemap_pairs.rs`.

## Frontend

- `src/lib/api/sitemap.ts` : wrappers `invoke`
- `src/lib/components/project/SitemapTab.svelte` : onglet dédié
  - Partie haute : liste des paires + CRUD inline (label, 2 URLs sitemap, 2 query params)
  - Partie basse : pour la paire sélectionnée, boutons **Ping** et **Diff** + résultats
- `ProjectDetail.svelte` : ajout du tab `"sitemap"` entre Docker et Agents
- `types/index.ts` : ajout `SitemapPair`, `PingResult`, `DiffResult`, `PingReport`, `DiffReport`

Affichage résultats :
- Tableau URL | statut | (mode ping: code HTTP + durée) | (mode diff: OK / différent / orphelin + taille)
- Clic sur une ligne "différent" → modal avec diff unifié (texte monospace)

## Edge cases

- Sitemap inaccessible → erreur globale remontée, run avorté.
- URL présente d'un seul côté → marquée `orphan_ref` / `orphan_check`, pas de diff.
- HTTP 5xx/timeout sur une URL → KO individuel, run continue.
- Sitemap avec >1000 URLs → pas de limite dure, barre de progression l'indique.

## Hors scope V1

- Sitemapindex (nested).
- Normalisation HTML / règles d'exclusion.
- Historique persisté des runs.
- Export du rapport (CSV, JSON).
- Authentification (cookies, headers custom).
