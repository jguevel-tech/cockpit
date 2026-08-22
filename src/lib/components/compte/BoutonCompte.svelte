<script lang="ts">
  /**
   * Le bouton du compte, dans l'en-tete.
   *
   * Il repond a une question qu'on se pose tout le temps et a laquelle rien ne repondait :
   * « est-ce que je suis connecte, et sous quel compte ? ». Connecte, il montre le portrait ;
   * sinon, une silhouette et une infobulle qui invite a se connecter.
   */
  import { compte, seDeconnecter } from "../../stores/compte";
  import { demanderConfirmation } from "../../stores/confirm";
  import { notify } from "../../stores/toast";
  import { trad } from "../../i18n";
  import { portal } from "../../actions/portal";
  import Portrait from "./Portrait.svelte";
  import EcranConnexion from "./EcranConnexion.svelte";
  import ProfilCompte from "./ProfilCompte.svelte";

  let menuOuvert = $state(false);
  let connexionOuverte = $state(false);
  let profilOuvert = $state(false);

  const connecte = $derived($compte?.connecte === true);
  const nom = $derived($compte?.nom || $compte?.email || "");

  function auClic() {
    if (connecte) menuOuvert = !menuOuvert;
    else connexionOuverte = true;
  }

  function ouvrirLeProfil() {
    menuOuvert = false;
    profilOuvert = true;
  }

  async function partir() {
    menuOuvert = false;
    const daccord = await demanderConfirmation({
      message: $trad("compte.profil.confirmerDeconnexion"),
      action: $trad("compte.profil.seDeconnecter"),
      danger: true,
    });
    if (daccord && (await seDeconnecter())) notify($trad("compte.profil.deconnecte"), "success");
  }
</script>

<button
  class="header-btn compte-btn"
  onclick={auClic}
  title={connecte ? nom : $trad("compte.seConnecter")}
  aria-label={connecte ? nom : $trad("compte.seConnecter")}
>
  {#if connecte}
    <Portrait avatar={$compte?.avatar} initiales={$compte?.initiales} taille={22} />
  {:else}
    <span class="silhouette" aria-hidden="true">&#128100;</span>
  {/if}
</button>

{#if menuOuvert}
  <!-- Le voile ferme le menu au premier clic ailleurs. `portal` parce que les conteneurs de
       l'application sont isoles : un overlay reste enfant serait peint dessous. -->
  <div class="voile" use:portal role="presentation" onclick={() => (menuOuvert = false)}></div>
  <div class="menu" use:portal>
    <div class="menu-tete">
      <Portrait avatar={$compte?.avatar} initiales={$compte?.initiales} taille={36} />
      <div class="menu-qui">
        {#if $compte?.nom}<strong>{$compte.nom}</strong>{/if}
        <span class="menu-email">{$compte?.email}</span>
      </div>
    </div>
    <button class="menu-item" onclick={ouvrirLeProfil}>{$trad("compte.monProfil")}</button>
    <!-- « Se deconnecter » ici et pas une seconde entree vers le profil : deux libelles qui
         menent au meme ecran font chercher la difference qui n'existe pas. -->
    <button class="menu-item" onclick={partir}>{$trad("compte.profil.seDeconnecter")}</button>
  </div>
{/if}

{#if connexionOuverte}
  <EcranConnexion onClose={() => (connexionOuverte = false)} />
{/if}
{#if profilOuvert}
  <ProfilCompte onClose={() => (profilOuvert = false)} />
{/if}

<style>
  .compte-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    overflow: hidden;
  }
  .silhouette {
    font-size: 0.95rem;
    line-height: 1;
    filter: grayscale(1);
    opacity: 0.75;
  }
  .voile {
    position: fixed;
    inset: 0;
    z-index: 90;
  }
  .menu {
    position: fixed;
    top: 3.1rem;
    right: 0.75rem;
    z-index: 91;
    min-width: 15rem;
    padding: 0.4rem;
    /* Token OPAQUE : sous image de fond, un `--bg-*` laisserait voir au travers. */
    background: var(--surface-base);
    border: 1px solid var(--border-color);
    border-radius: var(--radius);
    box-shadow: 0 12px 30px -10px rgba(0, 0, 0, 0.6);
  }
  .menu-tete {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.6rem 0.7rem;
    border-bottom: 1px solid var(--border-color);
    margin-bottom: 0.35rem;
  }
  .menu-qui {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .menu-qui strong {
    font-size: 0.87rem;
  }
  .menu-email {
    color: var(--text-muted);
    font-size: 0.76rem;
    overflow-wrap: anywhere;
  }
  .menu-item {
    display: block;
    width: 100%;
    padding: 0.45rem 0.6rem;
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-primary);
    font: inherit;
    font-size: 0.86rem;
    text-align: left;
    cursor: pointer;
  }
  .menu-item:hover {
    background: var(--accent-soft);
  }
</style>
