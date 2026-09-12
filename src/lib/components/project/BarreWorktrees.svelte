<script lang="ts">
  /// Les dossiers de travail du projet, au-dessus des onglets de terminal.
  ///
  /// **CE QU'ELLE REND VISIBLE : DANS QUELLE BRANCHE ON TAPE.** Cockpit savait deja ouvrir un
  /// terminal dans un worktree, mais rien ne le disait a l'ecran — et taper une commande dans
  /// le mauvais dossier est l'erreur que ca coute le plus cher de decouvrir apres coup.
  ///
  /// Un dossier = une couleur = un lot de terminaux avec sa disposition en volets. On passe
  /// d'un sujet a l'autre comme on change d'onglet.
  import { trad } from "../../i18n";
  import type { Groupe } from "../../terminaux/worktrees";

  interface Props {
    groupes: Groupe[];
    /// Chemin du dossier affiche. `null` tant que rien n'est choisi.
    actif: string | null;
    /// Couleur par chemin, distribuee en amont pour qu'elles soient toutes differentes.
    couleurs: Map<string, string>;
    surChoix: (chemin: string) => void;
    surCreer: () => void;
    surMenu: (evenement: MouseEvent, groupe: Groupe) => void;
  }

  let { groupes, actif, couleurs, surChoix, surCreer, surMenu }: Props = $props();
</script>

<!-- La barre n'existe pas quand le projet n'est pas un depot git : pas de bande vide, pas de
     bouton qui ne mene nulle part. -->
{#if groupes.length > 0}
  <div class="barre" role="tablist" aria-label={$trad("worktree.barreTitre")}>
    {#each groupes as groupe (groupe.chemin)}
      <button
        class="puce"
        class:actif={actif === groupe.chemin}
        style:--teinte={couleurs.get(groupe.chemin) ?? "var(--accent)"}
        role="tab"
        aria-selected={actif === groupe.chemin}
        title={groupe.chemin}
        onclick={() => surChoix(groupe.chemin)}
        oncontextmenu={(e) => surMenu(e, groupe)}
      >
        <span class="point" aria-hidden="true"></span>
        <span class="nom">{groupe.libelle}</span>
        <!-- Le compte dit où sont les terminaux SANS avoir a ouvrir chaque dossier. Zero est
             affiché aussi : un dossier vide est justement celui où l'on va entrer. -->
        <span class="compte" aria-hidden="true">{groupe.terminaux.length}</span>
      </button>
    {/each}
    <button class="ajouter" onclick={surCreer} title={$trad("worktree.creerAide")}>
      +
    </button>
  </div>
{/if}

<style>
  .barre {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px 0;
    flex-wrap: wrap;
  }

  .puce {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 8px;
    border: 1px solid var(--border-color);
    border-radius: 999px;
    /* Fond OPAQUE : la barre peut se trouver au-dessus d'une image de fond. */
    background: var(--surface-base, var(--bg-tertiary));
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
    max-width: 220px;
  }
  .puce:hover { color: var(--text-primary); }
  /* **LE DOSSIER ACTIF SE LIT A LA COULEUR, PAS SEULEMENT AU CONTRASTE.** Un liseré de sa
     teinte, repris sur le volet actif : c'est le meme reperage aux deux endroits. */
  .puce.actif {
    color: var(--text-primary);
    border-color: var(--teinte);
    box-shadow: inset 0 0 0 1px var(--teinte);
  }
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
  .compte {
    padding: 0 4px;
    border-radius: 999px;
    background: var(--bg-tertiary);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .puce.actif .compte { color: var(--text-secondary); }

  .ajouter {
    width: 24px;
    height: 24px;
    border: 1px dashed var(--border-color);
    border-radius: 999px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
  }
  .ajouter:hover {
    color: var(--text-primary);
    border-color: var(--text-muted);
  }
</style>
