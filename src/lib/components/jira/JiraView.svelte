<script lang="ts">
  /**
   * Mes tickets Jira. `projet` absent : vue globale avec filtre ; present : filtre fixe
   * (onglet d'un projet). Chaque ticket montre ses branches locales ; les branches sans
   * ticket sont listees a part, avec de quoi creer le ticket qui leur manque.
   */
  import { onMount } from "svelte";
  import { trad } from "../../i18n";
  import { notify } from "../../stores/toast";
  import { activeView } from "../../stores/ui";
  import {
    jiraConfig, jiraLiaisons, jiraMesTickets, jiraBranches, jiraDemarrer,
    type BrancheJira, type LiaisonJira, type TicketJira,
  } from "../../api/jira";
  import {
    grouperParCategorie, liaisonDuTicket, clesDuFiltre, rapprocher, type BrancheOrpheline,
  } from "../../jira/tickets";
  import TicketDetail from "./TicketDetail.svelte";
  import NouveauTicket from "./NouveauTicket.svelte";

  let { projet = null }: { projet?: string | null } = $props();

  let configure: boolean | null = $state(null);
  let liaisons: LiaisonJira[] = $state([]);
  let filtre = $state("");
  let tickets: TicketJira[] = $state([]);
  let chargement = $state(false);
  let ouvert: string | null = $state(null);
  let creation = $state(false);
  let depuisBranche: BrancheOrpheline | null = $state(null);
  let branchesParProjet: Record<string, BrancheJira[]> = $state({});
  let occupe = $state(false);

  const groupes = $derived(grouperParCategorie(tickets));
  const ticketOuvert = $derived(tickets.find((t) => t.cle === ouvert) ?? null);
  const cleParDefaut = $derived(clesDuFiltre(filtre, liaisons)?.[0] ?? "");
  const rapprochement = $derived(rapprocher(branchesParProjet));

  /** `null` : on ne sait pas (projet non lie ou branches illisibles), pas « aucune ». */
  function branchesDe(t: TicketJira): string[] | null {
    const l = liaisonDuTicket(t, liaisons);
    if (!l || !branchesParProjet[l.projet]) return null;
    return rapprochement.parTicket[t.cle] ?? [];
  }

  onMount(async () => {
    filtre = projet ?? "";
    try {
      const c = await jiraConfig();
      configure = !!c.url && c.jeton_pose;
      if (!configure) return;
      liaisons = await jiraLiaisons();
      await rafraichir();
    } catch (e) {
      notify(String(e));
    }
  });

  async function rafraichir() {
    chargement = true;
    try {
      [tickets] = await Promise.all([jiraMesTickets(clesDuFiltre(filtre, liaisons)), chargerBranches()]);
    } catch (e) {
      notify(String(e));
    } finally {
      chargement = false;
    }
  }

  /** Un projet sans dossier sur cette machine est simplement absent : rien a signaler. */
  async function chargerBranches() {
    const projets = liaisons.filter((l) => !filtre || l.projet === filtre).map((l) => l.projet);
    const lus = await Promise.all(projets.map((p) => jiraBranches(p).then((b) => [p, b] as const, () => null)));
    branchesParProjet = Object.fromEntries(lus.filter((x) => x !== null));
  }

  async function creerBranche(t: TicketJira) {
    const l = liaisonDuTicket(t, liaisons);
    if (!l) return;
    occupe = true;
    try {
      const d = await jiraDemarrer(l.projet, t.cle);
      notify($trad(d.creee ? "jira.brancheCreee" : "jira.brancheReprise", { branche: d.branche }), "success");
      if (d.erreur_transition) notify($trad("jira.transitionEchouee", { erreur: d.erreur_transition }));
      await rafraichir();
    } catch (e) {
      notify(String(e));
    } finally {
      occupe = false;
    }
  }

  function ouvrirCreation(b: BrancheOrpheline | null) {
    depuisBranche = b;
    creation = true;
  }
</script>

{#snippet groupe(titre: string, liste: TicketJira[])}
  <section class="card">
    <div class="card-head"><h3>{titre} <span class="badge">{liste.length}</span></h3></div>
    {#if liste.length === 0}
      <p class="empty">{$trad("jira.aucun")}</p>
    {:else}
      {#each liste as t (t.cle)}
        {@const branches = branchesDe(t)}
        <div class="ligne" class:actif={ouvert === t.cle}>
          <button class="ouvrir" onclick={() => (ouvert = t.cle)}>
            <span class="cle">{t.cle}</span>
            <span class="resume">{t.resume}</span>
            <span class="meta">{t.type_ticket}</span>
            <span class="meta">{t.priorite}</span>
            <span class="meta">{t.statut}</span>
            <span class="meta">{t.maj_le.slice(0, 10)}</span>
          </button>
          {#if branches === null}
            <span></span>
          {:else if branches.length === 0}
            <button class="btn small alerte" disabled={occupe} title={$trad("jira.sansBranche")} onclick={() => creerBranche(t)}>
              {$trad("jira.creerBranche")}
            </button>
          {:else}
            <span class="meta" title={branches.join("\n")}>⎇ {branches.length}</span>
          {/if}
        </div>
      {/each}
    {/if}
  </section>
{/snippet}

<div class="jira">
  {#if configure === false}
    <section class="card">
      <p>{$trad("jira.nonConfigure")}</p>
      <button class="btn" onclick={() => activeView.set("settings")}>{$trad("jira.ouvrirReglages")}</button>
    </section>
  {:else if configure}
    <div class="barre">
      {#if !projet}
        <select class="input" bind:value={filtre} onchange={rafraichir}>
          <option value="">{$trad("jira.tousLesProjets")}</option>
          {#each liaisons as l (l.projet)}
            <option value={l.projet}>{l.projet} ({l.cles.join(", ")})</option>
          {/each}
        </select>
      {/if}
      <button class="btn" onclick={rafraichir} disabled={chargement}>{$trad("common.refresh")}</button>
      <button class="btn primary" onclick={() => ouvrirCreation(null)}>{$trad("jira.nouveau")}</button>
    </div>

    <div class="contenu" class:avec-detail={!!ouvert}>
      <div class="stack">
        {@render groupe($trad("jira.enCours"), groupes.enCours)}
        {@render groupe($trad("jira.aFaire"), groupes.aFaire)}
        {#if Object.keys(branchesParProjet).length > 0}
          <section class="card">
            <div class="card-head">
              <h3>{$trad("jira.branchesSansTicket")} <span class="badge">{rapprochement.orphelines.length}</span></h3>
            </div>
            {#if rapprochement.orphelines.length === 0}
              <p class="empty">{$trad("jira.aucuneOrpheline")}</p>
            {:else}
              {#each rapprochement.orphelines as b (b.projet + "/" + b.nom)}
                <div class="orpheline">
                  <code class="resume">{b.nom}</code>
                  {#if !projet}<span class="meta">{b.projet}</span>{/if}
                  <button class="btn small" onclick={() => ouvrirCreation(b)}>{$trad("jira.creerTicket")}</button>
                </div>
              {/each}
            {/if}
          </section>
        {/if}
      </div>
      {#if ouvert}
        <TicketDetail
          cle={ouvert}
          liaison={ticketOuvert ? liaisonDuTicket(ticketOuvert, liaisons) : null}
          branches={ticketOuvert ? branchesDe(ticketOuvert) : null}
          onClose={() => (ouvert = null)}
          onChange={rafraichir}
        />
      {/if}
    </div>
    {#if creation}
      <NouveauTicket
        {liaisons}
        cleParDefaut={depuisBranche ? (liaisons.find((l) => l.projet === depuisBranche?.projet)?.cles[0] ?? "") : cleParDefaut}
        {depuisBranche}
        onClose={() => (creation = false)}
        onCree={(cle) => { creation = false; ouvert = cle; void rafraichir(); }}
      />
    {/if}
  {/if}
</div>

<style>
  .jira { flex: 1; min-width: 0; }
  .barre { display: flex; gap: 0.6rem; margin-bottom: 1rem; }
  .barre select { max-width: 20rem; }
  .contenu { display: grid; grid-template-columns: 1fr; gap: 1rem; align-items: start; }
  .contenu.avec-detail { grid-template-columns: 1fr minmax(20rem, 30rem); }
  .ligne {
    display: grid; grid-template-columns: 1fr 7rem; gap: 0.6rem; align-items: center;
    padding-right: 0.5rem; border-radius: 4px; font-size: 0.85rem;
  }
  .ligne:hover, .ligne.actif { background: var(--bg-tertiary); }
  .ouvrir {
    display: grid; grid-template-columns: 7rem 1fr 6rem 5rem 7rem 6rem; gap: 0.6rem; min-width: 0;
    width: 100%; padding: 0.4rem 0.5rem; text-align: left; font: inherit;
    background: none; border: none; color: var(--text-primary); cursor: pointer;
  }
  .alerte { color: var(--warning); }
  .orpheline {
    display: grid; grid-template-columns: 1fr auto auto; gap: 0.6rem; align-items: center;
    padding: 0.3rem 0.5rem; font-size: 0.85rem;
  }
  .cle { font-weight: 600; }
  .resume { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
