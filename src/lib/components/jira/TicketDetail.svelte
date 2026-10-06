<script lang="ts">
  /** Le panneau d'un ticket : statut, demarrage, temps, commentaires. */
  import { trad } from "../../i18n";
  import { notify } from "../../stores/toast";
  import { openUrl } from "../../api/workspace";
  import {
    jiraTicket, jiraTransitions, jiraTransitionner, jiraCommenter, jiraSaisirTemps,
    jiraDemarrer, jiraApercuBranche,
    type DetailTicketJira, type LiaisonJira, type TransitionJira,
  } from "../../api/jira";

  let { cle, liaison, branches = null, onClose, onChange }: {
    cle: string;
    liaison: LiaisonJira | null;
    /** Branches locales qui citent ce ticket ; `null` : inconnu. */
    branches?: string[] | null;
    onClose: () => void;
    onChange: () => void;
  } = $props();

  let detail: DetailTicketJira | null = $state(null);
  let transitions: TransitionJira[] = $state([]);
  let apercu = $state("");
  let commentaire = $state("");
  let duree = $state("");
  let commentaireTemps = $state("");
  let occupe = $state(false);

  $effect(() => {
    // Lu de facon synchrone : sinon l'effet ne suit pas `liaison` quand elle arrive
    // apres coup (un `await` avant la lecture la sort du suivi reactif de Svelte).
    const l = liaison;
    void charger(cle, l);
  });

  async function charger(c: string, l: LiaisonJira | null) {
    detail = null;
    transitions = [];
    apercu = "";
    try {
      const d = await jiraTicket(c);
      if (c !== cle) return;
      detail = d;
      const tr = await jiraTransitions(c);
      if (c !== cle) return;
      transitions = tr;
      if (l) {
        const a = await jiraApercuBranche(l.gabarit, d.ticket.cle, d.ticket.type_ticket, d.ticket.resume);
        if (c !== cle) return;
        apercu = a;
      }
    } catch (e) {
      notify(String(e));
    }
  }

  async function agir(action: (c: string) => Promise<unknown>, succes: string) {
    const c = cle;
    occupe = true;
    try {
      await action(c);
      notify(succes, "success");
      if (c === cle) await charger(c, liaison);
      onChange();
    } catch (e) {
      notify(String(e));
    } finally {
      occupe = false;
    }
  }

  const transitionner = (t: TransitionJira) =>
    agir((c) => jiraTransitionner(c, t.id), $trad("jira.statutChange", { statut: t.vers }));

  const commenter = () =>
    agir(async (c) => {
      await jiraCommenter(c, commentaire);
      commentaire = "";
    }, $trad("jira.commentaireAjoute"));

  const saisirTemps = () =>
    agir(async (c) => {
      await jiraSaisirTemps(c, duree, commentaireTemps.trim() || null);
      duree = "";
      commentaireTemps = "";
    }, $trad("jira.tempsSaisi"));

  async function demarrer() {
    if (!liaison) return;
    const c = cle;
    occupe = true;
    try {
      const d = await jiraDemarrer(liaison.projet, c);
      notify($trad(d.creee ? "jira.brancheCreee" : "jira.brancheReprise", { branche: d.branche }), "success");
      if (d.erreur_transition) notify($trad("jira.transitionEchouee", { erreur: d.erreur_transition }));
      if (c === cle) await charger(c, liaison);
      onChange();
    } catch (e) {
      notify(String(e));
    } finally {
      occupe = false;
    }
  }
</script>

<aside class="detail card">
  <div class="tete">
    <strong>{cle}</strong>
    <button class="icon-btn" onclick={onClose} aria-label={$trad("jira.fermer")}>✕</button>
  </div>

  {#if !detail}
    <p class="empty">{$trad("jira.chargement")}</p>
  {:else}
    {@const t = detail.ticket}
    <h3>{t.resume}</h3>
    <p class="meta"><span class="badge">{t.statut}</span> {t.type_ticket} · {t.priorite}</p>
    <button class="btn small ghost" onclick={() => openUrl(t.url)}>{$trad("jira.ouvrirDansJira")}</button>

    <div class="bloc">
      <span class="field-label">{$trad("jira.changerStatut")}</span>
      <div class="boutons">
        {#each transitions as tr (tr.id)}
          <button class="btn small" disabled={occupe} onclick={() => transitionner(tr)}>{tr.nom}</button>
        {/each}
      </div>
    </div>

    <div class="bloc">
      {#if liaison}
        <button class="btn primary" disabled={occupe} onclick={demarrer}>{$trad("jira.demarrer")}</button>
        {#if apercu}<code>{apercu}</code>{/if}
        {#if branches}
          <span class="field-label">{$trad("jira.branchesDuTicket", { n: branches.length })}</span>
          {#each branches as b (b)}<code>{b}</code>{/each}
        {/if}
      {:else}
        <p class="field-hint">{$trad("jira.demarrerSansProjet")}</p>
      {/if}
    </div>

    {#if t.description}<pre class="texte">{t.description}</pre>{/if}

    <div class="bloc">
      <span class="field-label">{$trad("jira.saisirTemps")}</span>
      <div class="ligne">
        <input class="input duree" bind:value={duree} placeholder={$trad("jira.dureeExemple")} />
        <input class="input" bind:value={commentaireTemps} placeholder={$trad("jira.commentaireTemps")} />
        <button class="btn" disabled={occupe || !duree.trim()} onclick={saisirTemps}>{$trad("jira.enregistrerTemps")}</button>
      </div>
    </div>

    <div class="bloc">
      <span class="field-label">{$trad("jira.commentaires", { n: detail.commentaires.length })}</span>
      {#each detail.commentaires as c}
        <div class="commentaire">
          <div class="meta">{c.auteur} · {c.cree_le.slice(0, 16).replace("T", " ")}</div>
          <pre class="texte">{c.corps}</pre>
        </div>
      {/each}
      <textarea class="input" rows="3" bind:value={commentaire}></textarea>
      <button class="btn" disabled={occupe || !commentaire.trim()} onclick={commenter}>{$trad("jira.commenter")}</button>
    </div>
  {/if}
</aside>

<style>
  .detail { display: flex; flex-direction: column; gap: 0.8rem; max-height: calc(100vh - 8rem); overflow-y: auto; }
  .tete { display: flex; justify-content: space-between; align-items: center; }
  h3 { margin: 0; font-size: 1rem; }
  .meta { color: var(--text-secondary); font-size: 0.8rem; margin: 0; }
  .bloc { display: flex; flex-direction: column; gap: 0.4rem; }
  .boutons { display: flex; flex-wrap: wrap; gap: 0.4rem; }
  .ligne { display: flex; gap: 0.4rem; }
  .duree { width: 6rem; flex: none; }
  .texte { white-space: pre-wrap; font: inherit; font-size: 0.85rem; margin: 0; }
  .commentaire { border-left: 2px solid var(--border-color); padding-left: 0.6rem; }
  code { font-size: 0.8rem; color: var(--text-secondary); }
</style>
