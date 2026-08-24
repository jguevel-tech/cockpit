/**
 * Producteur de notices : alertes systeme (disque plein, memoire saturee, CPU sature).
 *
 * Echantillonne les metriques toutes les 60 s, INDEPENDAMMENT de la vue Monitoring
 * (dont le live ne tourne que quand elle est affichee). Anti-spam par construction :
 * - une seule notice par condition, a id stable (sys:disk:/home, sys:mem, sys:cpu) ;
 * - CPU et memoire doivent depasser le seuil PLUSIEURS echantillons d'affilee
 *   (un pic de compilation n'est pas une alerte) ;
 * - hysteresis : la notice est retiree quand on redescend NETTEMENT sous le seuil,
 *   pas au premier echantillon limite — sinon elle clignoterait.
 *
 * Les CORPS des notices etaient ecrits en francais dans le code, unite comprise : la cloche
 * parlait francais a un utilisateur anglophone. L'audit i18n ne les voyait pas — un audit
 * vert ne prouve rien.
 */
import { getSystemMetrics } from "../api/system";
import { pushNotice, removeNotice } from "./notifications";
import { openSystem } from "./ui";
import { translate } from "../i18n";
import { signalerErreur } from "./errors";

const SAMPLE_MS = 60_000;

const DISK_ALERT = 90; // %
const DISK_CLEAR = 88;

const MEM_ALERT = 92;
const MEM_CLEAR = 85;
const MEM_SUSTAIN = 3; // echantillons consecutifs (3 min)

const CPU_ALERT = 95;
const CPU_CLEAR = 80;
const CPU_SUSTAIN = 5; // 5 min

let memStreak = 0;
let cpuStreak = 0;
let memAlerted = false;
let cpuAlerted = false;
const diskAlerted = new Set<string>();

export function startSystemAlerts(): () => void {
  check();
  const timer = setInterval(check, SAMPLE_MS);
  return () => clearInterval(timer);
}

async function check() {
  let m;
  try {
    m = await getSystemMetrics();
  } catch (e) {
      signalerErreur("systemAlerts.check", String(e));
    console.error("systemAlerts:", e);
    return;
  }

  // --- Disques : etat instantane (un disque ne se remplit pas par pic) ---
  for (const d of m.disks) {
    const id = `sys:disk:${d.mount}`;
    if (d.percent >= DISK_ALERT && !diskAlerted.has(d.mount)) {
      diskAlerted.add(d.mount);
      pushNotice({
        id,
        kind: "warning",
        title: translate("alerts.diskFull", { mount: d.mount }),
        body: translate("alerts.diskBody", {
          percent: String(Math.round(d.percent)),
          free: formatGb(d.free),
        }),
        createdAt: new Date().toISOString(),
        dismissible: true,
        action: { label: translate("alerts.seeMonitoring"), run: openSystem },
      });
    } else if (d.percent < DISK_CLEAR && diskAlerted.has(d.mount)) {
      diskAlerted.delete(d.mount);
      removeNotice(id);
    }
  }

  // --- Memoire : soutenue ---
  memStreak = m.memory.percent >= MEM_ALERT ? memStreak + 1 : 0;
  if (memStreak >= MEM_SUSTAIN && !memAlerted) {
    memAlerted = true;
    pushNotice({
      id: "sys:mem",
      kind: "warning",
      title: translate("alerts.memoryFull"),
      body: translate("alerts.memoryBody", { percent: String(Math.round(m.memory.percent)) }),
      createdAt: new Date().toISOString(),
      dismissible: true,
      action: { label: translate("alerts.seeMonitoring"), run: openSystem },
    });
  } else if (memAlerted && m.memory.percent < MEM_CLEAR) {
    memAlerted = false;
    removeNotice("sys:mem");
  }

  // --- CPU : soutenu ---
  cpuStreak = m.cpu.usage_percent >= CPU_ALERT ? cpuStreak + 1 : 0;
  if (cpuStreak >= CPU_SUSTAIN && !cpuAlerted) {
    cpuAlerted = true;
    pushNotice({
      id: "sys:cpu",
      kind: "warning",
      title: translate("alerts.cpuFull"),
      body: translate("alerts.cpuBody", { percent: String(Math.round(m.cpu.usage_percent)) }),
      createdAt: new Date().toISOString(),
      dismissible: true,
      action: { label: translate("alerts.seeMonitoring"), run: openSystem },
    });
  } else if (cpuAlerted && m.cpu.usage_percent < CPU_CLEAR) {
    cpuAlerted = false;
    removeNotice("sys:cpu");
  }
}

/// « Go » n'est pas « GB » : l'unite passe par le catalogue, comme le reste de la phrase.
function formatGb(bytes: number): string {
  return translate("alerts.gigabytes", { n: (bytes / 1024 / 1024 / 1024).toFixed(1) });
}
