import { translate } from "../i18n";

export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "Ko", "Mo", "Go", "To"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const v = bytes / Math.pow(1024, i);
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
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
