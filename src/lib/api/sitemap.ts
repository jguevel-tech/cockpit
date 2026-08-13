import { invoke } from "@tauri-apps/api/core";
import type { SitemapPair, SitemapPairInput, PingReport, DiffReport } from "../types";

export const getSitemapPairs = (project: string): Promise<SitemapPair[]> =>
  invoke("get_sitemap_pairs", { project });

// Les cles envoyees a invoke restent en camelCase (converties auto vers snake_case cote Rust).
export const createSitemapPair = (project: string, input: SitemapPairInput): Promise<SitemapPair> =>
  invoke("create_sitemap_pair", {
    project,
    label: input.label,
    sitemapRefUrl: input.sitemap_ref_url,
    sitemapCheckUrl: input.sitemap_check_url,
    refQuery: input.ref_query,
    checkQuery: input.check_query,
    limitUrls: input.limit_urls,
  });

export const updateSitemapPair = (id: number, input: SitemapPairInput): Promise<SitemapPair> =>
  invoke("update_sitemap_pair", {
    id,
    label: input.label,
    sitemapRefUrl: input.sitemap_ref_url,
    sitemapCheckUrl: input.sitemap_check_url,
    refQuery: input.ref_query,
    checkQuery: input.check_query,
    limitUrls: input.limit_urls,
  });

export const deleteSitemapPair = (id: number): Promise<void> =>
  invoke("delete_sitemap_pair", { id });

export const runSitemapPing = (pairId: number, skipUrls?: string[]): Promise<PingReport> =>
  invoke("run_sitemap_ping", { pairId, skipUrls: skipUrls ?? null });

export const runSitemapDiff = (pairId: number, skipPaths?: string[]): Promise<DiffReport> =>
  invoke("run_sitemap_diff", { pairId, skipPaths: skipPaths ?? null });

export const cancelSitemapCheck = (): Promise<void> =>
  invoke("cancel_sitemap_check");
