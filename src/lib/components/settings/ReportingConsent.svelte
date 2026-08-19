<script lang="ts">
  /**
   * Accord demande UNE fois, au premier lancement. Opt-in : rien ne part avant un oui
   * explicite. L'ecran dit exactement ce qui est transmis et ce qui ne l'est pas — c'est
   * la seule facon loyale de demander, et refuser ne coute aucune fonctionnalite.
   */
  import Modal from "../ui/Modal.svelte";
  import { trad } from "../../i18n";
  import { signalerErreur, setReportingConsent,
    setReportingUser,
    machineReport } from "../../stores/errors";
  import type { MachineReport } from "../../types";
  import { onMount } from "svelte";

  let nom = $state("");
  let fiche: MachineReport | null = $state(null);
  let details = $state(false);

  onMount(async () => {
    try {
      fiche = await machineReport();
    } catch (e) {
      signalerErreur("reportingConsent.onMount", String(e));
      // La fiche n'est qu'illustrative ici : son absence ne doit pas bloquer l'ecran.
      console.warn(String(e));
    }
  });

  async function accepter() {
    if (nom.trim()) await setReportingUser(nom.trim());
    await setReportingConsent(true);
  }

  async function refuser() {
    await setReportingConsent(false);
  }
</script>

<Modal title={$trad("consent.title")} width="560px" onClose={refuser}>
  <p class="body">{$trad("consent.body")}</p>
  <p class="body muted">{$trad("consent.notSent")}</p>

  <label class="field">
    <span>{$trad("consent.name")}</span>
    <input class="input" type="text" bind:value={nom} placeholder={$trad("consent.namePlaceholder")} />
  </label>

  <button class="btn small" onclick={() => (details = !details)}>
    {$trad("consent.seeData")}
  </button>
  {#if details && fiche}
    <ul class="fiche">
      <li>{$trad("settings.app.version")} : {fiche.app_version}</li>
      <li>{$trad("consent.distro")} : {fiche.distro}</li>
      <li>{$trad("consent.audio")} : {fiche.audio_server}</li>
      <li>pw-record : {fiche.pw_record || "—"}</li>
      <li>tmux : {fiche.tmux || "—"}</li>
      <li>{$trad("consent.packaging")} : {fiche.packaging}</li>
    </ul>
  {/if}

  <div class="actions">
    <button class="btn" onclick={refuser}>{$trad("consent.refuse")}</button>
    <button class="btn primary" onclick={accepter}>{$trad("consent.accept")}</button>
  </div>
</Modal>

<style>
  .body { margin: 0 0 0.75rem; color: var(--text-primary); line-height: 1.5; }
  .muted { color: var(--text-secondary); font-size: 0.9rem; }
  .field { display: flex; flex-direction: column; gap: 0.3rem; margin: 0.75rem 0; }
  .field span { color: var(--text-secondary); font-size: 0.9rem; }
  .fiche {
    margin: 0.5rem 0 0; padding: 0.5rem 1.25rem; list-style: disc;
    color: var(--text-secondary); font-size: 0.85rem; line-height: 1.6;
  }
  .actions { display: flex; justify-content: flex-end; gap: 0.5rem; margin-top: 1.25rem; }
</style>
