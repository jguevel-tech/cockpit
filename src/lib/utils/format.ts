import { translate } from "../i18n";

/**
 * Une taille en octets, lisible.
 *
 * **Les unites ne s'ecrivent pas pareil dans les deux langues** : « 50 o » et « 1,2 Mo » se
 * lisent « 50 B » et « 1.2 MB » en anglais. Elles vivent donc dans les catalogues, comme le
 * reste des libelles.
 *
 * Et il n'y a qu'UNE table : la meme existait en quatre exemplaires — deux en francais, deux en
 * anglais — donc la meme application montrait « 50 o » dans les fichiers et « 50 B » dans les
 * process, quelle que soit la langue choisie. Deux copies d'une table de libelles finissent
 * toujours par ne plus dire la meme chose.
 */
const UNITES = ["size.b", "size.kb", "size.mb", "size.gb", "size.tb"] as const;

export function formatBytes(bytes: number): string {
  if (bytes <= 0) return `0 ${translate(UNITES[0])}`;
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNITES.length - 1);
  const v = bytes / Math.pow(1024, i);
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${translate(UNITES[i])}`;
}

/**
 * Duree de fonctionnement, en clair.
 *
 * Ce texte etait fabrique cote Rust (`format!("{}j {}h {}m", ...)`), donc affiche en
 * francais a un utilisateur anglais. Le backend rend maintenant des SECONDES et la mise en
 * forme vit ici, dans les catalogues.
 */
export function formatUptime(secs: number): string {
  const total = Math.max(0, Math.floor(secs));
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  const mins = Math.floor((total % 3600) / 60);
  return days > 0
    ? translate("sys.uptimeDays", { days, hours, mins })
    : translate("sys.uptimeHours", { hours, mins });
}
