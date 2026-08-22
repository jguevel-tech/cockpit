<script lang="ts">
  /**
   * Le profil, dans l'application.
   *
   * Il repond aux questions qu'on se pose sur son compte sans avoir a ouvrir un navigateur :
   * qui suis-je, a quoi je ressemble, quelles machines sont connectees, ou en est la
   * synchronisation, et comment je pars.
   *
   * Ce qui se change ici passe par le SERVEUR : le nom et l'avatar suivent le compte, pas la
   * machine. Une modification locale qui ne remonterait pas donnerait deux visages selon le
   * poste.
   */
  import Modal from "../ui/Modal.svelte";
  import Portrait from "./Portrait.svelte";
  import { trad } from "../../i18n";
  import {
    compte,
    dernierRefus,
    definirNom,
    deposerAvatar,
    retirerAvatar,
    seDeconnecter,
    etatSynchro,
    synchroEnCours,
    synchroniser,
    rafraichirEtatSynchro,
  } from "../../stores/compte";
  import { machines as listerMachines, type Machine } from "../../api/compte";
  import { demanderConfirmation } from "../../stores/confirm";
  import { notify } from "../../stores/toast";
  import { signalerErreur } from "../../stores/errors";
  import { texteDuRefus } from "../../stores/refusCompte";
  import { open as ouvrirUnFichier } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  let { onClose }: { onClose: () => void } = $props();

  let nom = $state($compte?.nom ?? "");
  let enCours = $state(false);
  let mesMachines: Machine[] = $state([]);
  let machineCourante: string | null = $state(null);
  let machinesLues = $state(false);

  const SYSTEMES: Record<string, string> = { linux: "Linux", macos: "macOS", windows: "Windows" };

  onMount(() => {
    void rafraichirEtatSynchro();
    void chargerLesMachines();
  });

  async function chargerLesMachines() {
    try {
      const [liste, courante] = await listerMachines();
      mesMachines = liste;
      machineCourante = courante;
    } catch (e) {
      // Hors ligne, la liste n'est pas disponible : le reste du profil l'est, et l'ecran le dit.
      signalerErreur("compte.machines", String(e));
    } finally {
      machinesLues = true;
    }
  }

  async function enregistrerLeNom() {
    if (enCours || nom === ($compte?.nom ?? "")) return;
    enCours = true;
    try {
      if (await definirNom(nom.trim())) notify($trad("compte.profil.nomEnregistre"), "success");
    } finally {
      enCours = false;
    }
  }

  async function changerLAvatar() {
    if (enCours) return;
    const choix = await ouvrirUnFichier({
      multiple: false,
      filters: [{ name: $trad("compte.profil.images"), extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    if (typeof choix !== "string") return;

    enCours = true;
    try {
      if (await deposerAvatar(choix)) {
        notify($trad("compte.profil.avatarEnregistre"), "success");
      } else {
        const cle = texteDuRefus($dernierRefus);
        if (cle) notify($trad(cle), "error");
      }
    } finally {
      enCours = false;
    }
  }

  async function enleverLAvatar() {
    if (enCours) return;
    enCours = true;
    try {
      await retirerAvatar();
    } finally {
      enCours = false;
    }
  }

  async function partir() {
    const daccord = await demanderConfirmation({
      message: $trad("compte.profil.confirmerDeconnexion"),
      action: $trad("compte.profil.seDeconnecter"),
      danger: true,
    });
    if (!daccord) return;
    if (await seDeconnecter()) {
      notify($trad("compte.profil.deconnecte"), "success");
      onClose();
    }
  }

  const dernierPassage = $derived(
    $etatSynchro?.dernier_passage ? new Date($etatSynchro.dernier_passage).toLocaleString() : null,
  );
</script>

<Modal title={$trad("compte.profil.titre")} width="480px" {onClose}>
  <div class="tete">
    <Portrait avatar={$compte?.avatar} initiales={$compte?.initiales} taille={64} />
    <div class="qui">
      <strong>{$compte?.nom || $compte?.email}</strong>
      <span class="email">{$compte?.email}</span>
      <div class="actions-portrait">
        <button class="btn small" onclick={changerLAvatar} disabled={enCours}>
          {$trad("compte.profil.changerAvatar")}
        </button>
        {#if $compte?.avatar}
          <button class="btn small" onclick={enleverLAvatar} disabled={enCours}>
            {$trad("compte.profil.retirerAvatar")}
          </button>
        {/if}
      </div>
    </div>
  </div>

  <label class="field">
    <span>{$trad("compte.profil.nom")}</span>
    <input
      class="input"
      type="text"
      bind:value={nom}
      maxlength="120"
      placeholder={$trad("compte.profil.nomExemple")}
      onblur={enregistrerLeNom}
      onkeydown={(e) => { if (e.key === "Enter") enregistrerLeNom(); }}
    />
  </label>
  <p class="aide">{$trad("compte.profil.nomAide")}</p>

  <h3>{$trad("compte.profil.synchro")}</h3>
  <p class="ligne">
    <span>
      {#if $synchroEnCours}{$trad("settings.compte.synchroEnCours")}
      {:else if dernierPassage}{dernierPassage}
      {:else}{$trad("settings.compte.synchroJamais")}{/if}
      {#if ($etatSynchro?.en_attente ?? 0) > 0}
        <span class="attente">{$trad("settings.compte.enAttente")} : {$etatSynchro?.en_attente}</span>
      {/if}
    </span>
    <button class="btn small" onclick={() => void synchroniser()} disabled={$synchroEnCours}>
      {$trad("settings.compte.synchroniser")}
    </button>
  </p>

  <h3>{$trad("compte.profil.machines")}</h3>
  {#if !machinesLues}
    <p class="aide">{$trad("compte.profil.machinesChargement")}</p>
  {:else if mesMachines.length === 0}
    <p class="aide">{$trad("compte.profil.machinesIndisponibles")}</p>
  {:else}
    <ul class="machines">
      {#each mesMachines as machine (machine.id)}
        <li>
          <div>
            <strong>{machine.nom}</strong>
            {#if machine.id === machineCourante}
              <span class="ici">{$trad("compte.profil.celleCi")}</span>
            {/if}
            <span class="detail">{SYSTEMES[machine.systeme] ?? machine.systeme}</span>
          </div>
        </li>
      {/each}
    </ul>
    <p class="aide">{$trad("compte.profil.machinesAide")}</p>
  {/if}

  <div class="pied">
    <button class="btn danger" onclick={partir}>{$trad("compte.profil.seDeconnecter")}</button>
  </div>
</Modal>

<style>
  .tete {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-bottom: 1.3rem;
  }
  .qui {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .qui strong {
    font-size: 1rem;
  }
  .email {
    color: var(--text-muted);
    font-size: 0.8rem;
    overflow-wrap: anywhere;
  }
  .actions-portrait {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.45rem;
  }
  .field {
    display: block;
  }
  .field span {
    display: block;
    margin-bottom: 0.3rem;
    font-size: 0.82rem;
    color: var(--text-secondary);
  }
  .field .input {
    width: 100%;
  }
  .aide {
    margin: 0.4rem 0 0;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  h3 {
    margin: 1.5rem 0 0.5rem;
    color: var(--text-secondary);
    font-size: 0.78rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .ligne {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.8rem;
    margin: 0;
    color: var(--text-primary);
    font-size: 0.86rem;
  }
  .attente {
    margin-left: 0.5rem;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  .machines {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .machines li {
    padding: 0.5rem 0;
    font-size: 0.87rem;
  }
  .machines li + li {
    border-top: 1px solid var(--border-color);
  }
  .ici {
    margin-left: 0.4rem;
    padding: 0.05rem 0.35rem;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.7rem;
  }
  .detail {
    display: block;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  .pied {
    display: flex;
    justify-content: flex-end;
    margin-top: 1.6rem;
    padding-top: 1rem;
    border-top: 1px solid var(--border-color);
  }
</style>
