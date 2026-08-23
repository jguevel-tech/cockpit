<script lang="ts">
  /**
   * Le compte, en PAGE et non en fenetre par-dessus l'application.
   *
   * Une fenetre modale convient a un geste qu'on finit tout de suite. Ici on lit, on change son
   * nom, on regarde ses machines, on revient : c'est un endroit, pas une interruption. La page
   * est aussi le SEUL endroit du compte — l'adresse du serveur y a rejoint le reste, pour qu'on
   * n'ait pas a chercher entre deux ecrans lequel porte quoi.
   *
   * Ce qui se change ici passe par le SERVEUR : le nom et l'image suivent le compte, pas la
   * machine. Une modification locale qui ne remonterait pas donnerait deux visages selon le
   * poste.
   */
  import Portrait from "./Portrait.svelte";
  import EcranConnexion from "./EcranConnexion.svelte";
  import { trad } from "../../i18n";
  import {
    compte,
    dernierRefus,
    definirNom,
    definirServeur,
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

  let nom = $state("");
  let serveur = $state("");
  let enCours = $state(false);
  let ecranOuvert = $state(false);
  let mesMachines: Machine[] = $state([]);
  let machineCourante: string | null = $state(null);
  let machinesLues = $state(false);

  const SYSTEMES: Record<string, string> = { linux: "Linux", macos: "macOS", windows: "Windows" };

  const connecte = $derived($compte?.connecte === true);

  // Les deux champs sont recopies UNE fois, a l'arrivee de l'etat. Les relier au magasin en
  // continu ecraserait la frappe en cours des que la synchronisation rafraichit le compte.
  let champsRemplis = false;
  $effect(() => {
    const etat = $compte;
    if (!etat || champsRemplis) return;
    nom = etat.nom ?? "";
    serveur = etat.serveur;
    champsRemplis = true;
  });

  // Un seul passage, des que le compte est CONNU. Pas `onMount` : l'etat du compte est lu de
  // maniere asynchrone au demarrage, donc au montage de cette page il vaut souvent encore
  // `null` — la liste des machines n'aurait alors jamais ete demandee. Le drapeau garantit
  // qu'on ne redemande rien a chaque battement du magasin : une liste de machines ne change
  // pas pendant qu'on la regarde, et c'est un appel reseau.
  let dejaDemande = false;
  $effect(() => {
    if (!connecte || dejaDemande) return;
    dejaDemande = true;
    void rafraichirEtatSynchro();
    void chargerLesMachines();
  });

  async function chargerLesMachines() {
    try {
      const [liste, courante] = await listerMachines();
      mesMachines = liste;
      machineCourante = courante;
    } catch (e) {
      // Hors ligne, la liste n'est pas disponible : le reste de la page l'est, et elle le dit.
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

  async function enregistrerLeServeur() {
    if (await definirServeur(serveur)) {
      champsRemplis = false;
      notify($trad("settings.compte.serveurEnregistre"), "success");
    } else if ($dernierRefus === "serveur_non_chiffre") {
      notify($trad("settings.compte.serveurNonChiffre"), "error");
    }
  }

  async function changerLImage() {
    if (enCours) return;
    const choix = await ouvrirUnFichier({
      multiple: false,
      filters: [
        {
          name: $trad("compte.profil.images"),
          extensions: ["png", "jpg", "jpeg", "webp", "gif"],
        },
      ],
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

  async function enleverLImage() {
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
    if (await seDeconnecter()) notify($trad("compte.profil.deconnecte"), "success");
  }

  const dernierPassage = $derived(
    $etatSynchro?.dernier_passage ? new Date($etatSynchro.dernier_passage).toLocaleString() : null,
  );
</script>

<div class="page">
  <div class="stack">
    <!-- L'identite tient lieu de titre : un titre pose au-dessus du panneau devient illisible
         des qu'une image de fond est active (meme raison que le titre des Parametres). -->
    <section class="card">
      {#if connecte}
        <div class="identite">
          <Portrait avatar={$compte?.avatar} initiales={$compte?.initiales} taille={72} />
          <div class="qui">
            <strong>{$compte?.nom || $compte?.email}</strong>
            <span class="email">{$compte?.email}</span>
            <div class="actions-image">
              <button class="btn small" onclick={changerLImage} disabled={enCours}>
                {$trad("compte.profil.changerAvatar")}
              </button>
              {#if $compte?.avatar}
                <button class="btn small" onclick={enleverLImage} disabled={enCours}>
                  {$trad("compte.profil.retirerAvatar")}
                </button>
              {/if}
            </div>
          </div>
        </div>

        <label class="champ">
          <span>{$trad("compte.profil.nom")}</span>
          <input
            class="input"
            type="text"
            bind:value={nom}
            maxlength="120"
            placeholder={$trad("compte.profil.nomExemple")}
            onblur={enregistrerLeNom}
            onkeydown={(e) => {
              if (e.key === "Enter") enregistrerLeNom();
            }}
          />
        </label>
        <p class="aide">{$trad("compte.profil.nomAide")}</p>
      {:else}
        <div class="card-head">
          <h3>{$trad("compte.titre")}</h3>
          <p>{$trad("compte.intro")}</p>
        </div>
        <p class="aide">{$trad("settings.compte.aucun")}</p>
        <div class="actions">
          <button class="btn primary" onclick={() => (ecranOuvert = true)}>
            {$trad("compte.seConnecter")}
          </button>
        </div>
      {/if}
    </section>

    {#if connecte}
      <section class="card">
        <div class="card-head">
          <h3>{$trad("compte.profil.synchro")}</h3>
        </div>
        <div class="field-row">
          <span class="field-label">{$trad("settings.compte.machine")}</span>
          <span class="field-value">{$compte?.appareil}</span>
        </div>
        <div class="field-row">
          <span class="field-label">{$trad("compte.profil.synchro")}</span>
          <span class="field-value">
            {#if $synchroEnCours}
              {$trad("settings.compte.synchroEnCours")}
            {:else if dernierPassage}
              {dernierPassage}
            {:else}
              {$trad("settings.compte.synchroJamais")}
            {/if}
            {#if ($etatSynchro?.en_attente ?? 0) > 0}
              <span class="attente"
                >{$trad("settings.compte.enAttente")} : {$etatSynchro?.en_attente}</span
              >
            {/if}
          </span>
        </div>
        <div class="actions">
          <button class="btn" onclick={() => void synchroniser()} disabled={$synchroEnCours}>
            {$trad("settings.compte.synchroniser")}
          </button>
        </div>
        <p class="aide">{$trad("settings.compte.cheminsAide")}</p>
      </section>

      <section class="card">
        <div class="card-head">
          <h3>{$trad("compte.profil.machines")}</h3>
        </div>
        {#if !machinesLues}
          <p class="aide">{$trad("compte.profil.machinesChargement")}</p>
        {:else if mesMachines.length === 0}
          <p class="aide">{$trad("compte.profil.machinesIndisponibles")}</p>
        {:else}
          <ul class="machines">
            {#each mesMachines as machine (machine.id)}
              <li>
                <strong>{machine.nom}</strong>
                {#if machine.id === machineCourante}
                  <span class="ici">{$trad("compte.profil.celleCi")}</span>
                {/if}
                <span class="detail">{SYSTEMES[machine.systeme] ?? machine.systeme}</span>
              </li>
            {/each}
          </ul>
          <p class="aide">{$trad("compte.profil.machinesAide")}</p>
        {/if}
      </section>
    {/if}

    <!-- L'adresse du serveur vit ICI et nulle part ailleurs : c'est une donnee de compte, et la
         chercher entre deux ecrans faisait perdre du temps. -->
    <section class="card">
      <div class="card-head">
        <h3>{$trad("settings.compte.serveur")}</h3>
        <p>{$trad("settings.compte.serveurAide")}</p>
      </div>
      <div class="ligne-serveur">
        <input class="input" type="url" bind:value={serveur} placeholder="https://…" />
        <button class="btn" onclick={enregistrerLeServeur}>{$trad("common.save")}</button>
      </div>
    </section>

    {#if connecte}
      <section class="card">
        <div class="actions">
          <button class="btn danger" onclick={partir}>
            {$trad("compte.profil.seDeconnecter")}
          </button>
        </div>
      </section>
    {/if}
  </div>
</div>

{#if ecranOuvert}
  <EcranConnexion onClose={() => (ecranOuvert = false)} />
{/if}

<style>
  .page {
    max-width: 1060px;
  }
  .identite {
    display: flex;
    align-items: center;
    gap: 1.1rem;
    margin-bottom: 1.4rem;
  }
  .qui {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
  }
  .qui strong {
    font-size: 1.05rem;
  }
  .email {
    color: var(--text-muted);
    font-size: 0.82rem;
    overflow-wrap: anywhere;
  }
  .actions-image {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.5rem;
  }
  .champ {
    display: block;
    max-width: 26rem;
  }
  .champ span {
    display: block;
    margin-bottom: 0.3rem;
    font-size: 0.82rem;
    color: var(--text-secondary);
  }
  .champ .input {
    width: 100%;
  }
  .ligne-serveur {
    display: flex;
    gap: 0.5rem;
    max-width: 34rem;
  }
  .ligne-serveur .input {
    flex: 1;
    min-width: 12rem;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .aide {
    margin: 0.45rem 0 0;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  .attente {
    margin-left: 0.6rem;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  .machines {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .machines li {
    padding: 0.55rem 0;
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
</style>
