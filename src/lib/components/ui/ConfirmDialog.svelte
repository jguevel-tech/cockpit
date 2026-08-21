<script lang="ts">
  /**
   * La fenetre de confirmation de l'application, montee UNE FOIS dans `App.svelte` — meme
   * modele que `Toast`. N'importe quel code appelle `demanderConfirmation()` sans avoir a
   * porter un etat ni un balisage : c'est ce qui a permis de remplacer les vingt-quatre
   * `confirm()` du systeme sans en oublier.
   *
   * Le clavier suit les habitudes de l'application : Echap annule (c'est `Modal` qui s'en
   * charge), Entree valide. Le bouton de validation prend le focus a l'ouverture, donc Entree
   * marche sans viser, et un utilisateur au clavier sait ou il est.
   *
   * ATTENTION : le bouton par defaut est celui qui VALIDE, y compris pour une suppression.
   * C'est assume — on ne demande une confirmation que pour un geste que l'utilisateur vient
   * de declencher, et Echap reste a portee. Ce qui protege ici, c'est que la question NOMME ce
   * qui va disparaitre.
   */
  import Modal from "./Modal.svelte";
  import { confirmation } from "../../stores/confirm";
  import { trad } from "../../i18n";
</script>

{#if $confirmation}
  {@const demande = $confirmation}
  <Modal title={$trad("confirm.title")} width="440px" onClose={() => demande.repondre(false)}>
    <p class="message">{demande.message}</p>
    <div class="actions">
      <button class="btn" onclick={() => demande.repondre(false)}>
        {$trad("common.cancel")}
      </button>
      <!-- svelte-ignore a11y_autofocus -->
      <button
        class="btn"
        class:danger={demande.danger}
        class:primary={!demande.danger}
        autofocus
        onclick={() => demande.repondre(true)}
        onkeydown={(e) => { if (e.key === "Enter") demande.repondre(true); }}
      >
        {demande.action}
      </button>
    </div>
  </Modal>
{/if}

<style>
  .message {
    margin: 0 0 1.1rem;
    color: var(--text-primary);
    font-size: 0.9rem;
    line-height: 1.5;
    /* Une question peut nommer un chemin long : on va a la ligne plutot que de deborder. */
    overflow-wrap: anywhere;
    /* Les messages existants contiennent parfois deux phrases separees par un saut de ligne. */
    white-space: pre-line;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
