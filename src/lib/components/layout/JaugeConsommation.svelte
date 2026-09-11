<script lang="ts">
  /// Ce qu'il reste avant la limite du fournisseur, dans l'en-tete.
  ///
  /// **CE QU'ON REPOND, C'EST « EST-CE QUE JE PEUX LANCER CE GROS TRAVAIL MAINTENANT ».** D'ou
  /// l'anneau qui porte la fenetre la plus SERREE (c'est elle qui coupera en premier) et, au
  /// clic, l'heure de remise a zero en relatif : « repart dans 2 h 14 » se lit d'un coup d'oeil,
  /// « repart a 18 h 42 » demande un calcul.
  ///
  /// **RIEN NE S'AFFICHE QUAND IL N'Y A RIEN A MESURER** : un fournisseur sans cette capacite,
  /// ou personne de connecte. Une jauge vide se lirait « consommation nulle ».
  import { onMount } from "svelte";
  import { portal } from "../../actions/portal";
  import { trad } from "../../i18n";
  import {
    consommation,
    suivreLaConsommation,
    rafraichirConsommation,
    fenetreLaPlusServree,
    aQuelqueChoseAMontrer,
  } from "../../stores/consommation";

  let ouvert = $state(false);

  const visible = $derived(aQuelqueChoseAMontrer($consommation));
  const pire = $derived(fenetreLaPlusServree($consommation));
  const pourcentage = $derived(pire ? Math.round(pire.pourcentage) : 0);

  /// Trois paliers, et pas un degrade continu : la couleur doit dire « tranquille », « ca
  /// approche » ou « ca va couper », pas afficher une nuance qu'on n'interprete pas.
  const niveau = $derived(pourcentage >= 85 ? "alerte" : pourcentage >= 60 ? "attention" : "calme");

  onMount(() => suivreLaConsommation());

  function basculer() {
    ouvert = !ouvert;
    // Ouvrir est un geste explicite : on redemande un chiffre a jour, sans attendre le minuteur.
    if (ouvert) void rafraichirConsommation(true);
  }

  /// L'anneau : un cercle dont on ne dessine qu'une part. `stroke-dasharray` porte la
  /// circonference, `stroke-dashoffset` ce qui reste a ne pas peindre.
  const CIRCONFERENCE = 2 * Math.PI * 7;
  const reste = $derived(CIRCONFERENCE * (1 - Math.min(pourcentage, 100) / 100));

  /// « dans 2 h 14 », « dans 3 j ». Les unites vivent dans le catalogue : elles s'ecrivent
  /// autrement en anglais, et l'audit de traduction ne sait pas voir une chaine de deux
  /// lettres.
  function quand(remise: number | null): string {
    if (remise === null) return $trad("conso.resetInconnu");
    const secondes = remise - Math.floor(Date.now() / 1000);
    if (secondes <= 60) return $trad("conso.imminent");
    const minutes = Math.floor(secondes / 60);
    if (minutes < 60) return $trad("conso.dansMinutes", { n: minutes });
    const heures = Math.floor(minutes / 60);
    if (heures < 24) {
      const reste = minutes % 60;
      // Les minutes sont completees a deux chiffres : « 1 h 3 » se lit mal, et se lit meme
      // faux — on comprend « 1 h 30 » une fraction de seconde.
      return reste === 0
        ? $trad("conso.dansHeures", { n: heures })
        : $trad("conso.dansHeuresMinutes", { h: heures, m: String(reste).padStart(2, "0") });
    }
    return $trad("conso.dansJours", { n: Math.floor(heures / 24) });
  }

  /// L'heure exacte, en second rideau : le relatif se lit, l'absolu se verifie.
  function heureExacte(remise: number | null): string {
    if (remise === null) return "";
    return new Date(remise * 1000).toLocaleString(undefined, {
      weekday: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function libelle(cle: string): string {
    return cle === "session" ? $trad("conso.session") : $trad("conso.semaine");
  }

  function aide(cle: string): string {
    return cle === "session" ? $trad("conso.sessionAide") : $trad("conso.semaineAide");
  }
</script>

{#if visible}
  <button
    class="jauge {niveau}"
    class:ouvert
    onclick={basculer}
    title={$trad("conso.infobulle", { nom: $consommation?.nom ?? "", n: pourcentage })}
    aria-label={$trad("conso.infobulle", { nom: $consommation?.nom ?? "", n: pourcentage })}
    aria-expanded={ouvert}
  >
    <svg viewBox="0 0 18 18" aria-hidden="true">
      <circle class="piste" cx="9" cy="9" r="7" />
      <circle
        class="part"
        cx="9"
        cy="9"
        r="7"
        stroke-dasharray={CIRCONFERENCE}
        stroke-dashoffset={reste}
      />
    </svg>
    <span class="chiffre">{pourcentage}&nbsp;%</span>
  </button>
{/if}

{#if ouvert}
  <div class="voile" role="presentation" use:portal onclick={() => (ouvert = false)}></div>
  <div class="panneau" role="dialog" aria-label={$trad("conso.titre")} use:portal>
    <div class="tete">
      <h3>{$trad("conso.titre")}</h3>
      <span class="fournisseur">{$consommation?.nom}</span>
    </div>

    {#each $consommation?.fenetres ?? [] as fenetre (fenetre.cle)}
      {@const part = Math.round(fenetre.pourcentage)}
      <div class="ligne">
        <div class="entete-ligne">
          <span class="libelle">{libelle(fenetre.cle)}</span>
          <span class="part-chiffre">{part}&nbsp;%</span>
        </div>
        <div class="barre" role="img" aria-label="{libelle(fenetre.cle)} : {part} %">
          <div
            class="remplissage {part >= 85 ? 'alerte' : part >= 60 ? 'attention' : 'calme'}"
            style="width: {Math.min(part, 100)}%"
          ></div>
        </div>
        <div class="dessous">
          <span class="aide">{aide(fenetre.cle)}</span>
          <span class="remise" title={heureExacte(fenetre.remise_a_zero)}>
            {$trad("conso.repart", { quand: quand(fenetre.remise_a_zero) })}
          </span>
        </div>
      </div>
    {/each}

    {#if $consommation?.probleme}
      <p class="probleme">{$trad("conso.probleme")} : {$consommation.probleme}</p>
    {/if}
  </div>
{/if}

<style>
  .jauge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 8px 0 5px;
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .jauge:hover,
  .jauge.ouvert {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .jauge svg {
    width: 18px;
    height: 18px;
    transform: rotate(-90deg);
  }
  .piste {
    fill: none;
    stroke: var(--border-color);
    stroke-width: 2.5;
  }
  .part {
    fill: none;
    stroke-width: 2.5;
    stroke-linecap: round;
    transition: stroke-dashoffset 240ms ease;
  }
  .calme .part { stroke: var(--success); }
  .attention .part { stroke: var(--warning); }
  .alerte .part { stroke: var(--error); }
  .alerte .chiffre { color: var(--error); }

  .voile { position: fixed; inset: 0; z-index: 90; }

  .panneau {
    position: fixed;
    z-index: 91;
    top: calc(var(--header-height) + 6px);
    right: 12px;
    width: 288px;
    padding: 14px;
    /* Surface flottante : fond OPAQUE, jamais --bg-* qui devient translucide sous wallpaper. */
    background: var(--surface-base, var(--bg-secondary));
    border: 1px solid var(--border-color);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
  }
  .tete {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 12px;
  }
  .tete h3 {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .fournisseur { font-size: 12px; color: var(--text-muted); }

  .ligne + .ligne { margin-top: 14px; }
  .entete-ligne {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 5px;
  }
  .libelle { font-size: 12px; color: var(--text-primary); }
  .part-chiffre {
    font-size: 12px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .barre {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-tertiary);
    overflow: hidden;
  }
  .remplissage {
    height: 100%;
    border-radius: 3px;
    transition: width 240ms ease;
  }
  .remplissage.calme { background: var(--success); }
  .remplissage.attention { background: var(--warning); }
  .remplissage.alerte { background: var(--error); }
  .dessous {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    margin-top: 5px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .remise { white-space: nowrap; }
  .probleme {
    margin: 12px 0 0;
    font-size: 11px;
    color: var(--error);
  }
</style>
