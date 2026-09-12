<script lang="ts">
  /// Le dossier de travail affiche, sur la ligne des onglets.
  ///
  /// **UN BOUTON, PAS UNE BARRE.** La premiere version posait une rangee de puces AU-DESSUS
  /// des onglets : elle disait bien les choses, mais elle coutait une ligne entiere en haut de
  /// l'ecran, et elle debordait des qu'un projet avait plus de trois branches ouvertes. Ici,
  /// la branche courante se lit en permanence — c'est elle qui compte, puisqu'elle dit ou l'on
  /// tape — et les autres sont a un clic.
  import { trad } from "../../i18n";

  interface Props {
    libelle: string;
    couleur: string | null;
    /// Nombre de dossiers de travail. A un seul, le bouton reste (il porte « nouveau »).
    nombre: number;
    surOuvrir: (evenement: MouseEvent) => void;
  }

  let { libelle, couleur, nombre, surOuvrir }: Props = $props();
</script>

<button
  class="selecteur"
  style:--teinte={couleur ?? "var(--accent)"}
  title={$trad("worktree.selecteurAide", { branche: libelle })}
  aria-label={$trad("worktree.selecteurAide", { branche: libelle })}
  onclick={surOuvrir}
>
  <span class="point" aria-hidden="true"></span>
  <span class="nom">{libelle}</span>
  {#if nombre > 1}<span class="chevron" aria-hidden="true">▾</span>{/if}
</button>

<style>
  .selecteur {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    max-width: 200px;
    padding: 0 8px;
    /* Le liseré porte la couleur du dossier : c'est le meme reperage que sur le volet actif. */
    border: 1px solid var(--teinte);
    border-radius: var(--radius-sm, 6px);
    /* Fond OPAQUE : la barre d'onglets peut se trouver au-dessus d'une image de fond. */
    background: var(--surface-base, var(--bg-tertiary));
    color: var(--text-primary);
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
  }
  .selecteur:hover { background: var(--bg-hover); }
  .point {
    width: 8px;
    height: 8px;
    flex: 0 0 auto;
    border-radius: 50%;
    background: var(--teinte);
  }
  .nom {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chevron { color: var(--text-muted); font-size: 9px; }
</style>
