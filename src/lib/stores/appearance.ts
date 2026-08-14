import { writable, derived, get } from "svelte/store";
import { setWallpaper, getWallpaper, clearWallpaper } from "../api/appearance";
import { notify } from "./toast";

/**
 * Apparence : palette, accent personnalise, image de fond.
 *
 * Deux mecanismes CSS, volontairement distincts (voir theme.css) :
 * - la classe `html.dark` porte la BASE (sombre ou claire) — c'est elle que lit xterm
 * - l'attribut `html[data-theme]` porte la PALETTE
 *
 * Ajouter une palette = un bloc dans theme.css + une entree dans THEMES ci-dessous.
 */

export interface ThemeDef {
  id: string;
  label: string;
  /** Determine `html.dark`, donc le theme du terminal. */
  base: "dark" | "light";
  /** Pastilles de l'apercu dans les reglages : fond, surface, accent. */
  preview: [string, string, string];
}

export const THEMES: ThemeDef[] = [
  { id: "dark", label: "Sombre", base: "dark", preview: ["#0e1015", "#161922", "#6d8dff"] },
  { id: "midnight", label: "Bleu nuit", base: "dark", preview: ["#0a0f1e", "#111830", "#5b9dff"] },
  { id: "plum", label: "Prune", base: "dark", preview: ["#14101c", "#1d182a", "#b57cff"] },
  { id: "forest", label: "Forêt", base: "dark", preview: ["#0b1310", "#121d18", "#4fc98a"] },
  { id: "ember", label: "Braise", base: "dark", preview: ["#14100d", "#1e1814", "#ff9d4d"] },
  { id: "light", label: "Clair", base: "light", preview: ["#f6f7f9", "#ffffff", "#4f7cff"] },
  { id: "paper", label: "Papier", base: "light", preview: ["#f7f4ee", "#fffdf8", "#b7791f"] },
];

const DEFAULTS = {
  theme: "dark",
  /** null = accent de la palette. Sinon un hex `#rrggbb`. */
  accent: null as string | null,
  /** Opacite des surfaces au-dessus de l'image, en %. 92 et non 82 : a 82 le texte sur une
   *  image chargee restait penible malgre le flou. Le curseur permet de descendre. */
  surfaceAlpha: 92,
  /** Voile au-dessus de l'image, 0..1. */
  wallpaperDim: 0.55,
  /** Flou de l'image, en px. */
  wallpaperBlur: 0,
  /** Eclat du verre depoli : le saturate() du backdrop-filter dope les couleurs de l'image
   *  derriere les panneaux. Sur une image tres coloree ca "sur-brille" — DESACTIVE par
   *  defaut (demande de Jimmy, 2026-08-14). */
  glassShine: false,
};

const KEY = "cockpit-appearance";

export const theme = writable<string>(DEFAULTS.theme);
export const accent = writable<string | null>(DEFAULTS.accent);
export const surfaceAlpha = writable<number>(DEFAULTS.surfaceAlpha);
export const wallpaperDim = writable<number>(DEFAULTS.wallpaperDim);
export const wallpaperBlur = writable<number>(DEFAULTS.wallpaperBlur);
export const glassShine = writable<boolean>(DEFAULTS.glassShine);
/** Data URL de l'image, ou null. Vit cote Rust (fichier), pas en localStorage. */
export const wallpaper = writable<string | null>(null);

/**
 * Base de la palette courante : "dark" ou "light".
 *
 * A consommer partout ou le CHOIX BINAIRE compte plutot que la palette — typiquement le
 * theme d'xterm, qui n'a que deux jeux de couleurs. Evite d'avoir a etendre un `Record`
 * a chaque nouvelle palette ajoutee.
 */
export const themeBase = derived(theme, (id): "dark" | "light" =>
  (THEMES.find((t) => t.id === id) ?? THEMES[0]).base
);

// --- Persistance des reglages legers (localStorage) ---
// L'image, elle, est un fichier cote Rust : trop lourde pour localStorage (quota ~5 Mo).

/// Filet contre un bug qui a coute une version (0.5.1 -> 0.5.2) : `subscribe()` de Svelte
/// declenche IMMEDIATEMENT son callback avec la valeur courante. Les abonnements etant
/// enregistres avant `loadSettings()`, ils sauvegardaient les valeurs par defaut et ecrasaient
/// les reglages de l'utilisateur AVANT qu'ils ne soient relus — le theme choisi revenait a
/// sombre a chaque redemarrage.
///
/// L'ordre est corrige, mais ce drapeau rend le module insensible a l'ordre : tant que le
/// chargement initial n'a pas eu lieu, aucune ecriture n'est possible.
let loaded = false;

function loadSettings() {
  if (typeof window === "undefined") return;
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) {
      // Migration depuis l'ancienne cle, quand le theme etait un simple "light"|"dark".
      const legacy = localStorage.getItem("cockpit-theme");
      if (legacy === "light" || legacy === "dark") theme.set(legacy);
      return;
    }
    const s = JSON.parse(raw);
    if (THEMES.some((t) => t.id === s.theme)) theme.set(s.theme);
    if (typeof s.accent === "string" && /^#[0-9a-f]{6}$/i.test(s.accent)) accent.set(s.accent);
    if (typeof s.surfaceAlpha === "number") surfaceAlpha.set(clamp(s.surfaceAlpha, 40, 100));
    if (typeof s.wallpaperDim === "number") wallpaperDim.set(clamp(s.wallpaperDim, 0, 0.95));
    if (typeof s.wallpaperBlur === "number") wallpaperBlur.set(clamp(s.wallpaperBlur, 0, 24));
    if (typeof s.glassShine === "boolean") glassShine.set(s.glassShine);
  } catch {
    // Reglages corrompus : on repart des valeurs par defaut plutot que de casser le demarrage.
  }
}

function saveSettings() {
  if (typeof window === "undefined" || !loaded) return;
  try {
    localStorage.setItem(
      KEY,
      JSON.stringify({
        theme: get(theme),
        accent: get(accent),
        surfaceAlpha: get(surfaceAlpha),
        wallpaperDim: get(wallpaperDim),
        wallpaperBlur: get(wallpaperBlur),
        glassShine: get(glassShine),
      })
    );
  } catch (e) {
    console.error("appearance: localStorage", e);
  }
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

// --- Application au DOM ---

function applyTheme(id: string) {
  if (typeof document === "undefined") return;
  const def = THEMES.find((t) => t.id === id) ?? THEMES[0];
  const root = document.documentElement;
  root.setAttribute("data-theme", def.id);
  // La classe reste la source de verite de la BASE : xterm et TerminalTab la lisent.
  root.classList.toggle("dark", def.base === "dark");
}

function applyAccent(hex: string | null) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  if (!hex) {
    root.style.removeProperty("--accent");
    root.style.removeProperty("--accent-hover");
    root.style.removeProperty("--accent-soft");
    return;
  }
  root.style.setProperty("--accent", hex);
  // Survol : on eclaircit de 12 % via color-mix plutot que de recalculer un hex a la main.
  root.style.setProperty("--accent-hover", `color-mix(in srgb, ${hex} 84%, white)`);
  root.style.setProperty("--accent-soft", `color-mix(in srgb, ${hex} 17%, transparent)`);
}

function applyWallpaperVars() {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--surface-alpha", `${get(surfaceAlpha)}%`);
  root.style.setProperty("--wallpaper-dim", String(get(wallpaperDim)));
  root.style.setProperty("--wallpaper-blur", `${get(wallpaperBlur)}px`);
  // 100 % = saturation neutre ; l'eclat (118 %) est un choix explicite de l'utilisateur
  root.style.setProperty("--glass-saturate", get(glassShine) ? "118%" : "100%");
  root.classList.toggle("has-wallpaper", get(wallpaper) !== null);
}

// L'ORDRE EST CRITIQUE — lire la note sur `loaded` plus haut avant de le modifier.
// 1. Charger d'abord : les stores portent alors les valeurs de l'utilisateur.
loadSettings();

// 2. S'abonner ensuite. Chaque `subscribe` se declenche immediatement, ce qui applique
//    les valeurs chargees au DOM — inutile d'appeler les `apply*` a la main.
theme.subscribe((id) => { applyTheme(id); saveSettings(); });
accent.subscribe((hex) => { applyAccent(hex); saveSettings(); });
surfaceAlpha.subscribe(() => { applyWallpaperVars(); saveSettings(); });
wallpaperDim.subscribe(() => { applyWallpaperVars(); saveSettings(); });
wallpaperBlur.subscribe(() => { applyWallpaperVars(); saveSettings(); });
glassShine.subscribe(() => { applyWallpaperVars(); saveSettings(); });
wallpaper.subscribe(() => applyWallpaperVars());

// 3. N'autoriser les ecritures qu'apres tout ca.
loaded = true;

/** Charge l'image persistee. Appele une fois au demarrage. */
export async function loadWallpaper() {
  try {
    wallpaper.set(await getWallpaper());
  } catch (e) {
    console.error("loadWallpaper", e);
  }
}

/** Bascule sombre <-> clair en gardant l'esprit de la palette courante quand c'est possible. */
export function toggleBase() {
  const current = THEMES.find((t) => t.id === get(theme)) ?? THEMES[0];
  const target = current.base === "dark" ? "light" : "dark";
  theme.set(THEMES.find((t) => t.base === target)?.id ?? "dark");
}

// --- Image de fond ---

/** Plus grande dimension conservee. Au-dela, on n'y gagne rien a l'ecran et le poids explose. */
const MAX_EDGE = 2560;

/**
 * Redimensionne et re-encode l'image choisie, puis en extrait la couleur dominante.
 *
 * Le redimensionnement est fait ICI et pas cote Rust parce que le canvas du WebView sait le
 * faire sans dependance : un JPEG de 8 Mo ressort typiquement sous 400 Ko en WebP, ce qui rend
 * le stockage en fichier et le rechargement au demarrage negligeables.
 */
async function processImage(dataUrl: string): Promise<{ dataUrl: string; accent: string }> {
  const img = new Image();
  img.src = dataUrl;
  await img.decode();

  const scale = Math.min(1, MAX_EDGE / Math.max(img.naturalWidth, img.naturalHeight));
  const w = Math.max(1, Math.round(img.naturalWidth * scale));
  const h = Math.max(1, Math.round(img.naturalHeight * scale));

  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("canvas 2d indisponible");
  ctx.drawImage(img, 0, 0, w, h);

  return { dataUrl: encode(canvas), accent: dominantColor(ctx, w, h) };
}

/**
 * Encode le canvas en verifiant le format REELLEMENT obtenu.
 *
 * `toDataURL` ne signale pas un type non supporte : la spec impose de retomber
 * silencieusement sur PNG. WebKitGTK n'encode pas le WebP, donc demander `image/webp`
 * produisait un PNG non compresse — une image de 4 Mo restait a 4 Mo, rechargee en base64
 * (x1.33) a chaque demarrage. On verifie donc le prefixe et on bascule sur JPEG, qui est
 * universellement supporte et parfaitement adapte a une photo de fond (la transparence
 * n'a aucun interet ici).
 */
function encode(canvas: HTMLCanvasElement): string {
  const webp = canvas.toDataURL("image/webp", 0.85);
  if (webp.startsWith("data:image/webp")) return webp;
  return canvas.toDataURL("image/jpeg", 0.85);
}

/**
 * Couleur dominante « vivante » de l'image.
 *
 * On echantillonne une grille (pas tous les pixels : inutile et lent), on ignore les pixels
 * quasi noirs ou quasi blancs — sur une image sombre ils ecrasent tout — puis on retient le
 * pixel le plus sature parmi les plus lumineux. Resultat : un accent qui ressort de l'image
 * au lieu d'une moyenne grisatre.
 */
function dominantColor(ctx: CanvasRenderingContext2D, w: number, h: number): string {
  const STEP = Math.max(1, Math.floor(Math.min(w, h) / 64));
  const { data } = ctx.getImageData(0, 0, w, h);
  let best = { score: -1, r: 109, g: 141, b: 255 };

  for (let y = 0; y < h; y += STEP) {
    for (let x = 0; x < w; x += STEP) {
      const i = (y * w + x) * 4;
      const r = data[i], g = data[i + 1], b = data[i + 2];
      if (data[i + 3] < 200) continue; // pixel transparent

      const max = Math.max(r, g, b), min = Math.min(r, g, b);
      const lum = (max + min) / 2;
      if (lum < 40 || lum > 225) continue; // trop sombre ou trop clair pour servir d'accent
      const sat = max === 0 ? 0 : (max - min) / max;

      // Sature d'abord, lumineux ensuite : un accent doit se voir sur fond sombre ET clair.
      const score = sat * 2 + (lum / 255);
      if (score > best.score) best = { score, r, g, b };
    }
  }

  const hex = (n: number) => n.toString(16).padStart(2, "0");
  return `#${hex(best.r)}${hex(best.g)}${hex(best.b)}`;
}

/**
 * Installe une image de fond depuis une data URL brute (fichier lu par le frontend).
 * `deriveAccent` : reprend la couleur dominante comme accent de l'interface.
 */
export async function applyWallpaper(rawDataUrl: string, deriveAccent: boolean) {
  const { dataUrl, accent: dominant } = await processImage(rawDataUrl);
  await setWallpaper(dataUrl);
  wallpaper.set(dataUrl);
  if (deriveAccent) accent.set(dominant);
}

export async function removeWallpaper() {
  try {
    await clearWallpaper();
    wallpaper.set(null);
  } catch (e) {
    notify(String(e));
  }
}

export function resetAppearance() {
  theme.set(DEFAULTS.theme);
  accent.set(DEFAULTS.accent);
  surfaceAlpha.set(DEFAULTS.surfaceAlpha);
  wallpaperDim.set(DEFAULTS.wallpaperDim);
  wallpaperBlur.set(DEFAULTS.wallpaperBlur);
}
