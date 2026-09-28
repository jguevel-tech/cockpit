<script lang="ts">
  /**
   * Choisir le profil d'une fenetre : un existant, ou un nouveau.
   *
   * Un profil deja ouvert est ramene par la coquille plutot que rouvert : un profil n'a
   * jamais qu'une fenetre.
   */
  import Modal from "../ui/Modal.svelte";
  import { trad } from "../../i18n";
  import { notify } from "../../stores/toast";
  import { choixDeProfilOuvert } from "../../stores/profil";
  import { ouvrirLeProfil, profilsDesFenetres, type EtatDesProfils } from "../../coquille";
  import { nomDeProfilValide } from "../../utils/profils";

  let etat = $state<EtatDesProfils | null>(null);
  let nouveau = $state("");
  const nouveauValide = $derived(nomDeProfilValide(nouveau));

  $effect(() => {
    profilsDesFenetres()
      .then((reponse) => (etat = reponse))
      .catch((e) => notify(String(e)));
  });

  function fermer() {
    choixDeProfilOuvert.set(false);
  }

  async function ouvrir(nom: string | null) {
    try {
      await ouvrirLeProfil(nom);
      fermer();
    } catch (e) {
      notify(String(e));
    }
  }

  function creer(evenement: SubmitEvent) {
    evenement.preventDefault();
    if (nouveauValide) void ouvrir(nouveau);
  }
</script>

<Modal title={$trad("profils.titre")} onClose={fermer}>
  {#if etat}
    <ul class="profils">
      {#each etat.profils as p (p.nom ?? "")}
        <li>
          <button class="profil" onclick={() => ouvrir(p.nom)}>
            <span>{p.nom ?? $trad("profils.defaut")}</span>
            {#if p.nom === etat.courant}
              <span class="etiquette">{$trad("profils.courant")}</span>
            {:else if p.ouvert}
              <span class="etiquette">{$trad("profils.ouvert")}</span>
            {/if}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  <form class="nouveau" onsubmit={creer}>
    <input
      bind:value={nouveau}
      placeholder={$trad("profils.nouveauPlaceholder")}
      aria-label={$trad("profils.nouveau")}
      aria-invalid={nouveau !== "" && !nouveauValide}
    />
    <button type="submit" class="btn-primary" disabled={!nouveauValide}>
      {$trad("profils.creer")}
    </button>
  </form>
  {#if nouveau !== "" && !nouveauValide}
    <p class="regle">{$trad("profils.regle")}</p>
  {/if}
</Modal>

<style>
  .profils { list-style: none; margin: 0 0 1rem; padding: 0; display: flex; flex-direction: column; gap: 0.25rem; }
  .profil {
    width: 100%; display: flex; justify-content: space-between; align-items: center;
    padding: 0.5rem 0.75rem; background: var(--bg-secondary); color: var(--text-primary);
    border: 1px solid var(--border-color); border-radius: 6px; cursor: pointer; text-align: left;
  }
  .profil:hover { border-color: var(--accent); }
  .etiquette { font-size: 0.75rem; color: var(--text-secondary); }
  .nouveau { display: flex; gap: 0.5rem; }
  .nouveau input { flex: 1; }
  .regle { margin: 0.5rem 0 0; font-size: 0.8rem; color: var(--error); }
</style>
