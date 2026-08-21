<script lang="ts">
  /**
   * L'ecran de connexion, montre au premier lancement et rappelable depuis les reglages.
   *
   * **« Continuer sans compte » est un vrai choix, pas une porte de sortie honteuse.** Cockpit
   * fonctionne entierement sans : le compte ne sert qu'a retrouver ses donnees sur une autre
   * machine. Cet ecran ne doit jamais donner l'impression du contraire.
   *
   * Deux chemins de connexion, et ils ne se ressemblent pas :
   * - le mot de passe se saisit ICI, sans navigateur ;
   * - Google passe par le navigateur, parce que Cockpit n'a pas de serveur HTTP et ne peut
   *   donc rien recevoir en retour. Le logiciel interroge le serveur jusqu'a obtenir son
   *   jeton : rien a recopier.
   */
  import Modal from "../ui/Modal.svelte";
  import { trad } from "../../i18n";
  import {
    dernierRefus,
    sInscrire,
    seConnecter,
    appairerParLeNavigateur,
  } from "../../stores/compte";
  import { openUrl } from "../../api/workspace";
  import { signalerErreur } from "../../stores/errors";
  import { onDestroy } from "svelte";

  let { onClose }: { onClose: () => void } = $props();

  let mode: "connexion" | "inscription" = $state("connexion");
  let email = $state("");
  let motDePasse = $state("");
  let enCours = $state(false);

  /** La demande d'appairage en cours, si l'utilisateur est parti par le navigateur. */
  let attente: { code: string; url: string } | null = $state(null);
  let arreterAttente: (() => void) | null = null;

  // La scrutation doit s'arreter si l'ecran disparait, sinon elle continue dans le vide.
  onDestroy(() => arreterAttente?.());

  const refus = $derived($dernierRefus);

  /**
   * Chaque motif rendu par le serveur a sa phrase. Une TABLE et non une cle construite a la
   * volee : le catalogue est type, donc ecrire les cles en toutes lettres fait verifier par le
   * compilateur qu'elles existent — dans les deux langues. Une cle assemblee passerait la
   * verification et manquerait a l'affichage.
   *
   * Un motif inconnu tombe sur un message general : le serveur peut en ajouter avant que le
   * logiciel les connaisse, et afficher une cle technique serait pire que rien.
   */
  const REFUS = {
    identifiants_invalides: "compte.refus.identifiants_invalides",
    adresse_deja_prise: "compte.refus.adresse_deja_prise",
    adresse_invalide: "compte.refus.adresse_invalide",
    mot_de_passe_trop_court: "compte.refus.mot_de_passe_trop_court",
    trop_de_tentatives: "compte.refus.trop_de_tentatives",
    reseau: "compte.refus.reseau",
    appairage_expire: "compte.refus.appairage_expire",
  } as const;

  const texteDuRefus = $derived(
    refus === null
      ? null
      : refus in REFUS
        ? $trad(REFUS[refus as keyof typeof REFUS])
        : $trad("compte.refus.serveur"),
  );

  async function valider(e: Event) {
    e.preventDefault();
    if (enCours) return;
    enCours = true;
    try {
      const ok =
        mode === "inscription"
          ? await sInscrire(email.trim(), motDePasse)
          : await seConnecter(email.trim(), motDePasse);
      if (ok) onClose();
    } finally {
      enCours = false;
    }
  }

  async function parGoogle() {
    if (enCours) return;
    enCours = true;
    try {
      const demande = await appairerParLeNavigateur();
      if (!demande) return;
      attente = { code: demande.demande.code, url: demande.demande.url };
      arreterAttente = demande.arreter;
      if (await demande.fini) onClose();
      else attente = null;
    } finally {
      enCours = false;
    }
  }

  function annulerAttente() {
    arreterAttente?.();
    arreterAttente = null;
    attente = null;
  }

  async function rouvrirLaPage() {
    if (!attente) return;
    try {
      await openUrl(attente.url);
    } catch (e) {
      signalerErreur("compte.ecran.rouvrir", String(e));
    }
  }
</script>

<Modal title={$trad("compte.titre")} width="470px" {onClose}>
  {#if attente}
    <p class="intro">{$trad("compte.attente.texte")}</p>
    <p class="code-ligne">
      <span class="code-libelle">{$trad("compte.attente.code")}</span>
      <code>{attente.code}</code>
    </p>
    <p class="aide">{$trad("compte.attente.verifier")}</p>
    <div class="actions">
      <button class="btn" onclick={annulerAttente}>{$trad("common.cancel")}</button>
      <button class="btn" onclick={rouvrirLaPage}>{$trad("compte.attente.rouvrir")}</button>
    </div>
  {:else}
    <p class="intro">{$trad("compte.intro")}</p>

    <div class="bascule" role="tablist">
      <button
        class="btn onglet"
        class:actif={mode === "connexion"}
        role="tab"
        aria-selected={mode === "connexion"}
        onclick={() => (mode = "connexion")}
      >
        {$trad("compte.seConnecter")}
      </button>
      <button
        class="btn onglet"
        class:actif={mode === "inscription"}
        role="tab"
        aria-selected={mode === "inscription"}
        onclick={() => (mode = "inscription")}
      >
        {$trad("compte.creer")}
      </button>
    </div>

    {#if texteDuRefus}
      <p class="refus">{texteDuRefus}</p>
    {/if}

    <form onsubmit={valider}>
      <label class="field">
        <span>{$trad("compte.email")}</span>
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input" type="email" bind:value={email} autocomplete="email" required autofocus />
      </label>
      <label class="field">
        <span>{$trad("compte.motDePasse")}</span>
        <input
          class="input"
          type="password"
          bind:value={motDePasse}
          autocomplete={mode === "inscription" ? "new-password" : "current-password"}
          required
        />
      </label>
      {#if mode === "inscription"}
        <p class="aide">{$trad("compte.longueurMinimale")}</p>
      {/if}
      <button class="btn primary large" type="submit" disabled={enCours}>
        {mode === "inscription" ? $trad("compte.creer") : $trad("compte.seConnecter")}
      </button>
    </form>

    <div class="separateur"><span>{$trad("compte.ou")}</span></div>

    <button class="btn large" onclick={parGoogle} disabled={enCours}>
      {$trad("compte.parGoogle")}
    </button>

    <button class="btn lien" onclick={onClose}>{$trad("compte.sansCompte")}</button>

    <!-- Dit une fois, a l'endroit ou l'utilisateur arrive, ce qui est transmis et ou le
         couper. Le retirer reviendrait a changer le comportement sans le dire. -->
    <p class="mention">{$trad("compte.mentionJournaux")}</p>
  {/if}
</Modal>

<style>
  .intro {
    margin: 0 0 1.2rem;
    color: var(--text-secondary);
    font-size: 0.88rem;
    line-height: 1.55;
  }
  .bascule {
    display: flex;
    gap: 0.35rem;
    margin-bottom: 1.1rem;
  }
  .onglet {
    flex: 1;
    justify-content: center;
  }
  .onglet.actif {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--text-primary);
  }
  .field {
    display: block;
    margin-bottom: 0.85rem;
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
    margin: -0.4rem 0 0.9rem;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  .refus {
    margin: 0 0 1rem;
    padding: 0.55rem 0.75rem;
    border: 1px solid var(--danger);
    border-radius: var(--radius-sm);
    color: var(--danger);
    font-size: 0.84rem;
  }
  .large {
    width: 100%;
    justify-content: center;
  }
  .separateur {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    margin: 1rem 0;
    color: var(--text-muted);
    font-size: 0.78rem;
  }
  .separateur::before,
  .separateur::after {
    content: "";
    flex: 1;
    height: 1px;
    background: var(--border-color);
  }
  .lien {
    width: 100%;
    justify-content: center;
    margin-top: 0.8rem;
    border-color: transparent;
    background: transparent;
    color: var(--text-secondary);
  }
  .mention {
    margin: 1.1rem 0 0;
    color: var(--text-muted);
    font-size: 0.74rem;
    line-height: 1.5;
  }
  .code-ligne {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    margin: 0 0 0.6rem;
  }
  .code-libelle {
    color: var(--text-secondary);
    font-size: 0.84rem;
  }
  .code-ligne code {
    padding: 0.2rem 0.55rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    background: var(--surface-base);
    font-size: 0.95rem;
    letter-spacing: 0.1em;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1.2rem;
  }
</style>
