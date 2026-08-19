/**
 * Traduction de l'interface. Francais par defaut, anglais au choix.
 *
 * Pas de librairie : un magasin Svelte et deux catalogues suffisent, et la reactivite
 * marche partout ou `$t` est lisible. Le francais (`fr.ts`) est la reference : le type
 * du catalogue en decoule, donc **une cle absente de `en.ts` est une erreur TypeScript**,
 * attrapee par `npm run check`. C'est ce qui evite d'avoir a y penser.
 *
 * Dans un composant :  {$trad("header.settings")}
 * Hors composant    :  translate("toast.copied")
 */
import { derived, get, writable } from "svelte/store";
import { fr, type Catalog } from "./fr";
import { en } from "./en";

export type Locale = "fr" | "en";
export type { Catalog };

const CATALOGS: Record<Locale, Catalog> = { fr, en };
const STORAGE_KEY = "cockpit-locale";

export const LOCALES: { id: Locale; label: string }[] = [
  { id: "fr", label: "Francais" },
  { id: "en", label: "English" },
];

function initial(): Locale {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "fr" || saved === "en") return saved;
  } catch (e) {
    // Un localStorage indisponible ne doit pas empecher l'application de demarrer.
    console.warn("langue: lecture impossible,", String(e));
  }
  return "fr";
}

export const locale = writable<Locale>(initial());

locale.subscribe((value) => {
  try {
    localStorage.setItem(STORAGE_KEY, value);
  } catch (e) {
    console.warn("langue: enregistrement impossible,", String(e));
  }
  if (typeof document !== "undefined") document.documentElement.lang = value;
});

export function setLocale(value: Locale) {
  locale.set(value);
}

/** Remplace les {reperes} d'un modele par les valeurs fournies. */
function fill(template: string, params?: Record<string, string | number>): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (whole, key) =>
    key in params ? String(params[key]) : whole,
  );
}

function lookup(lang: Locale, key: keyof Catalog): string {
  // Repli sur le francais : une traduction anglaise vide affiche le texte de reference
  // plutot qu'une cle brute a l'ecran.
  return CATALOGS[lang][key] || fr[key];
}

/** Traduit hors composant (magasins, modules utilitaires). */
export function translate(key: keyof Catalog, params?: Record<string, string | number>): string {
  return fill(lookup(get(locale), key), params);
}

/** Traduit dans un composant : {$trad("cle")}. Ni `t` ni `tr` : `t` sert trop souvent de
 * variable de boucle (elle masquerait le magasin) et `tr` est une balise HTML, que Svelte
 * prendrait pour un composant. */
export const trad = derived(
  locale,
  (lang) =>
    (key: keyof Catalog, params?: Record<string, string | number>): string =>
      fill(lookup(lang, key), params),
);

/**
 * Choisit entre singulier et pluriel : `tn("sidebar.projects", 3)` lit les cles
 * `sidebar.projects.one` et `sidebar.projects.other`, et expose {n}.
 */
export const tradN = derived(
  locale,
  (lang) =>
    (key: string, n: number, params?: Record<string, string | number>): string => {
      const form = `${key}.${n === 1 ? "one" : "other"}` as keyof Catalog;
      return fill(lookup(lang, form), { n, ...params });
    },
);
