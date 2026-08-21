<script lang="ts">
  /**
   * La carte « Compte » des reglages : etat de la connexion, machine, serveur.
   *
   * Elle dit toujours ce qu'il en est, y compris quand il n'y a pas de compte — un ecran qui
   * se contente de ne rien afficher laisse croire a une panne.
   */
  import { trad } from "../../i18n";
  import { compte, seDeconnecter, definirServeur, dernierRefus } from "../../stores/compte";
  import { demanderConfirmation } from "../../stores/confirm";
  import { notify } from "../../stores/toast";
  import EcranConnexion from "./EcranConnexion.svelte";

  let ecranOuvert = $state(false);
  let serveur = $state("");
  let serveurCharge = false;

  // La valeur du champ suit celle du backend tant que personne n'y a touche : sans ca, le champ
  // resterait vide apres un rechargement et donnerait l'impression qu'aucun serveur n'est
  // configure.
  $effect(() => {
    const etat = $compte;
    if (etat && !serveurCharge) {
      serveur = etat.serveur;
      serveurCharge = true;
    }
  });

  async function deconnecter() {
    const daccord = await demanderConfirmation({
      message: $trad("settings.compte.confirmerDeconnexion"),
      action: $trad("settings.compte.seDeconnecter"),
      danger: true,
    });
    if (!daccord) return;
    if (await seDeconnecter()) notify($trad("settings.compte.deconnecte"), "success");
  }

  async function enregistrerServeur() {
    if (await definirServeur(serveur)) {
      serveurCharge = false;
      notify($trad("settings.compte.serveurEnregistre"), "success");
    } else if ($dernierRefus === "serveur_non_chiffre") {
      notify($trad("settings.compte.serveurNonChiffre"), "error");
    }
  }
</script>

<section class="card">
  <div class="card-head">
    <h3>{$trad("settings.compte.title")}</h3>
    <p>{$trad("settings.compte.help")}</p>
  </div>

  {#if $compte?.connecte}
    <div class="field-row">
      <span class="field-label">{$trad("settings.compte.connecteEnTant")}</span>
      <span class="valeur">{$compte.email}</span>
    </div>
    <div class="field-row">
      <span class="field-label">{$trad("settings.compte.machine")}</span>
      <span class="valeur">{$compte.appareil}</span>
    </div>
    <div class="actions">
      <button class="btn danger" onclick={deconnecter}>{$trad("settings.compte.seDeconnecter")}</button>
    </div>
  {:else}
    <p class="etat">{$trad("settings.compte.aucun")}</p>
    <div class="actions">
      <button class="btn primary" onclick={() => (ecranOuvert = true)}>
        {$trad("settings.compte.seConnecter")}
      </button>
    </div>
  {/if}

  <div class="field-row">
    <span class="field-label">{$trad("settings.compte.serveur")}</span>
    <input class="input" type="url" bind:value={serveur} placeholder="https://…" />
    <button class="btn" onclick={enregistrerServeur}>{$trad("common.save")}</button>
  </div>
  <p class="aide">{$trad("settings.compte.serveurAide")}</p>
</section>

{#if ecranOuvert}
  <EcranConnexion onClose={() => (ecranOuvert = false)} />
{/if}

<style>
  .etat {
    margin: 0 0 0.9rem;
    color: var(--text-secondary);
    font-size: 0.86rem;
  }
  .valeur {
    color: var(--text-primary);
    font-size: 0.88rem;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    margin: 0.4rem 0 1rem;
  }
  .aide {
    margin: 0.35rem 0 0;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  .field-row .input {
    flex: 1;
    min-width: 12rem;
  }
</style>
