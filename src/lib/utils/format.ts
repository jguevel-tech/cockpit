export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "Ko", "Mo", "Go", "To"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const v = bytes / Math.pow(1024, i);
  return `${v >= 100 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}
