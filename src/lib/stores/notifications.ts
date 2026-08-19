import { writable, derived } from "svelte/store";
import { signalerErreur } from "./errors";

/**
 * Centre de notifications.
 *
 * Architecture : une notice n'est JAMAIS persistee. Elle est recreee a chaque lancement par
 * son producteur (l'updater aujourd'hui, une source distante demain). Seul l'etat utilisateur
 * — lu, ecarte — est conserve en localStorage, indexe par l'`id` de la notice.
 *
 * Consequence pratique : une notice peut porter une `action` sous forme de callback, ce qui
 * serait impossible si on serialisait l'objet. Et ajouter une source revient a appeler
 * `pushNotice()` depuis un nouveau module, sans toucher a ce fichier ni au panneau.
 */

export type NoticeKind = "update" | "info" | "warning" | "error";

export interface NoticeAction {
  label: string;
  /** Peut etre async ; le panneau affiche un etat occupe pendant l'execution. */
  run: () => void | Promise<void>;
}

export interface Notice {
  /** Identite logique et stable (ex. `update:0.3.0`) : sert au dedoublonnage ET a l'etat lu. */
  id: string;
  kind: NoticeKind;
  title: string;
  /** Markdown, rendu par le panneau. */
  body?: string;
  /** ISO 8601. */
  createdAt: string;
  action?: NoticeAction;
  /** Une notice non ecartable reste jusqu'a ce que son producteur cesse de la poser. */
  dismissible?: boolean;
}

const READ_KEY = "cockpit-notices-read";
const DISMISSED_KEY = "cockpit-notices-dismissed";

function loadIds(key: string): Set<string> {
  if (typeof window === "undefined") return new Set();
  try {
    const raw = localStorage.getItem(key);
    const parsed = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((v) => typeof v === "string") : []);
  } catch (e) {
      signalerErreur("notifications.loadIds", String(e));
    // localStorage corrompu ou illisible : on repart d'un etat vide plutot que de casser l'UI.
    return new Set();
  }
}

function saveIds(key: string, ids: Set<string>) {
  if (typeof window === "undefined") return;
  try {
    localStorage.setItem(key, JSON.stringify([...ids]));
  } catch (e) {
      signalerErreur("notifications.saveIds", String(e));
    console.error("notifications: localStorage", e);
  }
}

const readIds = writable<Set<string>>(loadIds(READ_KEY));
const dismissedIds = writable<Set<string>>(loadIds(DISMISSED_KEY));

readIds.subscribe((ids) => saveIds(READ_KEY, ids));
dismissedIds.subscribe((ids) => saveIds(DISMISSED_KEY, ids));

/** Notices brutes, toutes sources confondues. Ne pas consommer directement : voir `notices`. */
const rawNotices = writable<Notice[]>([]);

let dismissedSnapshot = new Set<string>();
dismissedIds.subscribe((ids) => (dismissedSnapshot = ids));

/** Notices visibles, les plus recentes d'abord, enrichies de leur etat de lecture. */
export const notices = derived(
  [rawNotices, readIds, dismissedIds],
  ([list, read, dismissed]) =>
    list
      .filter((n) => !dismissed.has(n.id))
      .map((n) => ({ ...n, read: read.has(n.id) }))
      .sort((a, b) => b.createdAt.localeCompare(a.createdAt))
);

export const unreadCount = derived(notices, (list) => list.filter((n) => !n.read).length);

/**
 * Ajoute ou met a jour une notice. Idempotent : appeler plusieurs fois avec le meme `id`
 * ne cree pas de doublon, ce qui permet aux producteurs de re-poser leur notice a chaque
 * verification sans se soucier de l'etat courant.
 */
export function pushNotice(notice: Notice) {
  if (dismissedSnapshot.has(notice.id)) return;
  rawNotices.update((list) => {
    const i = list.findIndex((n) => n.id === notice.id);
    if (i === -1) return [...list, notice];
    const next = [...list];
    next[i] = notice;
    return next;
  });
}

/** Retire une notice posee par un producteur (ex. la mise a jour vient d'etre installee). */
export function removeNotice(id: string) {
  rawNotices.update((list) => list.filter((n) => n.id !== id));
}

/** Retire toutes les notices d'un producteur, identifiees par le prefixe de leur id. */
export function removeNoticesByPrefix(prefix: string) {
  rawNotices.update((list) => list.filter((n) => !n.id.startsWith(prefix)));
}

export function markRead(id: string) {
  readIds.update((ids) => new Set(ids).add(id));
}

export function markAllRead() {
  rawNotices.update((list) => {
    readIds.update((ids) => {
      const next = new Set(ids);
      list.forEach((n) => next.add(n.id));
      return next;
    });
    return list;
  });
}

/** Ecarte definitivement : le producteur ne pourra plus re-poser cette notice. */
export function dismiss(id: string) {
  dismissedIds.update((ids) => new Set(ids).add(id));
}
