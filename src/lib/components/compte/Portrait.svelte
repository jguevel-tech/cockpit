<script lang="ts">
  /**
   * L'avatar d'un compte, ou ses initiales.
   *
   * Jamais un rond vide : ca se lirait comme une image qui n'a pas charge. Le composant existe
   * parce que le portrait apparait a trois endroits — l'en-tete, le menu, le profil — et que
   * trois copies auraient fini par ne plus se ressembler.
   */
  let {
    avatar = null,
    initiales = null,
    taille = 28,
  }: { avatar?: string | null; initiales?: string | null; taille?: number } = $props();

  // Deux lettres au plus : au-dela ca ne tient pas dans le rond a petite taille.
  const lettres = $derived((initiales ?? "?").slice(0, 2));
</script>

{#if avatar}
  <img class="portrait" src={avatar} alt="" width={taille} height={taille} style:--taille="{taille}px" />
{:else}
  <span class="portrait initiales" style:--taille="{taille}px" aria-hidden="true">{lettres}</span>
{/if}

<style>
  .portrait {
    display: block;
    flex-shrink: 0;
    width: var(--taille);
    height: var(--taille);
    border-radius: 50%;
    object-fit: cover;
  }
  .initiales {
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent);
    color: #fff;
    /* La taille du texte suit celle du rond : une valeur fixe deborderait au petit format. */
    font-size: calc(var(--taille) * 0.38);
    font-weight: 650;
    letter-spacing: 0.02em;
    line-height: 1;
    text-transform: uppercase;
    user-select: none;
  }
</style>
