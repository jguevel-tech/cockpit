<script lang="ts">
  /**
   * La fenetre de saisie de l'application, montee UNE FOIS dans `App.svelte` — meme modele que
   * `ConfirmDialog`. N'importe quel code appelle `demanderTexte()` sans porter d'etat ni de
   * balisage.
   *
   * Elle existe parce que **le `prompt()` du navigateur n'existe pas dans la WebView de
   * macOS** : il rend `null` sans rien afficher, donc les boutons qui l'appelaient ne faisaient
   * RIEN sur un Mac. Voir `stores/saisie.ts`.
   *
   * Le clavier suit les habitudes de l'application : Echap annule (c'est `Modal` qui s'en
   * charge), Entree valide. Le champ prend le focus a l'ouverture et son contenu est
   * SELECTIONNE : on renomme en tapant, sans avoir a effacer d'abord.
   */
  import Modal from "./Modal.svelte";
  import { saisie } from "../../stores/saisie";
  import { trad } from "../../i18n";

  let texte = $state("");
  let champ: HTMLInputElement | undefined = $state(undefined);

  // A chaque nouvelle demande : on repart de sa valeur, et on selectionne. Sans ce suivi, une
  // deuxieme demande reafficherait ce qui a ete tape dans la premiere.
  $effect(() => {
    const demande = $saisie;
    if (!demande) return;
    texte = demande.valeur;
    // Apres le rendu du champ, sinon il n'existe pas encore.
    queueMicrotask(() => { champ?.focus(); champ?.select(); });
  });

  /** Un texte vide vaut annulation : valider un nom vide ne veut rien dire. */
  function valider(repondre: (t: string | null) => void) {
    const propre = texte.trim();
    repondre(propre === "" ? null : propre);
  }
</script>

{#if $saisie}
  {@const demande = $saisie}
  <Modal title={$trad("saisie.title")} width="440px" onClose={() => demande.repondre(null)}>
    <p class="message">{demande.message}</p>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      bind:this={champ}
      bind:value={texte}
      type="text"
      autofocus
      placeholder={demande.exemple}
      aria-label={demande.message}
      onkeydown={(e) => {
        if (e.key === "Enter") { e.preventDefault(); valider(demande.repondre); }
      }}
    />
    <div class="actions">
      <button class="btn" onclick={() => demande.repondre(null)}>
        {$trad("common.cancel")}
      </button>
      <button class="btn primary" disabled={texte.trim() === ""} onclick={() => valider(demande.repondre)}>
        {demande.action}
      </button>
    </div>
  </Modal>
{/if}

<style>
  .message {
    margin: 0 0 0.7rem;
    color: var(--text-primary);
    font-size: 0.9rem;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  input {
    width: 100%;
    padding: 0.45rem 0.6rem;
    margin-bottom: 1.1rem;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: 0.9rem;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
