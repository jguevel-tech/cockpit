<script lang="ts">
  /**
   * La liste de ce qui est DECLARE dans le namespace, et le detail d'un objet.
   *
   * **DEUX NIVEAUX, JAMAIS UN SEUL ECRAN QUI DEPLIE TOUT.** Une liste qui montre les pods sous
   * chaque objet fait defiler des centaines de lignes pour lire soixante-dix-neuf noms. On voit
   * donc les objets, on entre dans l'un d'eux, et on revient.
   *
   * **ET ON COMPTE DES OBJETS.** L'ecran a annonce « taches planifiees 264 » sur un namespace
   * qui en declare 79 : il comptait les pods qu'elles avaient laisses derriere elles.
   */
  import { trad } from "../../i18n";
  import K8sLignePod from "./K8sLignePod.svelte";
  import { formaterCpu, formaterRam, age, type Element, type Pod } from "../../k8s/vue";

  let {
    elements, maintenant, kubectl = false, choisi = null, surOuvrirPod, surShell, surSupprimer,
    surLancer, vide,
  }: {
    elements: Element[];
    maintenant: number;
    kubectl?: boolean;
    choisi?: Pod | null;
    surOuvrirPod: (pod: Pod, volet?: "logs" | "evenements" | "yaml") => void;
    surShell?: (pod: Pod) => void;
    surSupprimer?: (pods: Pod[]) => void;
    /// Lance une tache planifiee maintenant. Rend quand le cluster a repondu.
    surLancer?: (tache: string) => Promise<void>;
    vide: string;
  } = $props();

  /// Dans quel objet on est entre. Le nom suffit : la liste ne melange pas deux sortes qui
  /// porteraient le meme.
  let entre: string | null = $state(null);

  /// **L'OBJET OUVERT SE RELIT DANS LA LISTE A JOUR**, jamais gardé de côté : le flux met les
  /// pods a jour en continu, et une copie figee afficherait l'etat qu'il avait a l'ouverture.
  const ouvert = $derived(entre ? (elements.find((e) => e.nom === entre) ?? null) : null);

  /// La reference des jauges : le plus gros de l'ecran. Sans reference commune, deux barres de
  /// meme longueur diraient deux choses differentes.
  const cpuMax = $derived(Math.max(1, ...elements.map((e) => e.cpu ?? 0)));
  const ramMax = $derived(Math.max(1, ...elements.map((e) => e.ram ?? 0)));

  /// **UN POINT VERT SUR UNE TACHE QUI N'A JAMAIS TOURNE EST UN MENSONGE.** Le vert dit « ca
  /// tourne ». Une tache planifiee passe l'essentiel de sa vie sans aucun pod : son point reste
  /// donc gris, et ne devient vert que pendant qu'elle travaille.
  function couleurDe(e: Element): string {
    if (e.ennuyeux) return "mauvais";
    if (e.suspendu) return "eteint";
    return e.pods.some((p) => p.etat === "Running") ? "bon" : "eteint";
  }

  /// Un lancement en cours : le bouton le dit et ne repart pas sur un second clic, qui
  /// creerait un second travail.
  let lancement = $state(false);
  async function lancer(nom: string) {
    if (!surLancer || lancement) return;
    lancement = true;
    try { await surLancer(nom); } finally { lancement = false; }
  }

  /// Les pods de l'objet ouvert qui ont echoue : ce sont eux qu'on vient nettoyer.
  const enEchec = $derived(ouvert?.pods.filter((p) => p.ennuyeux) ?? []);

  function etatDe(e: Element): { texte: string; alerte: boolean } {
    if (e.sorte === "CronJob") {
      if (e.suspendu) return { texte: $trad("k8s.suspendu"), alerte: false };
      if (!e.dernier) return { texte: $trad("k8s.jamaisLance"), alerte: false };
      return { texte: $trad("k8s.lanceIlYA", { duree: age(e.dernier, maintenant) }), alerte: false };
    }
    return { texte: `${e.prets}/${e.voulus}`, alerte: e.prets < e.voulus };
  }
</script>

{#if ouvert}
  <div class="detail-objet">
    <div class="fil">
      <button class="retour" onclick={() => (entre = null)}>← {$trad("k8s.retourListe")}</button>
      <span class="fil-nom">{ouvert.nom}</span>
      {#if ouvert.sorte}<span class="sorte">{ouvert.sorte}</span>{/if}
    </div>

    <div class="faits">
      {#if ouvert.planification}
        <span class="fait" title={$trad("k8s.planification")}>
          <span class="cle">{$trad("k8s.planification")}</span>
          <span class="mono">{ouvert.planification}</span>
        </span>
      {/if}
      {#if ouvert.sorte === "CronJob"}
        <span class="fait">
          <span class="cle">{$trad("k8s.dernierLancement")}</span>
          <span>{ouvert.dernier ? age(ouvert.dernier, maintenant) : $trad("k8s.jamaisLance")}</span>
        </span>
      {:else}
        <span class="fait">
          <span class="cle">{$trad("k8s.replicas")}</span>
          <span class:incomplet={ouvert.prets < ouvert.voulus}>{ouvert.prets}/{ouvert.voulus}</span>
        </span>
      {/if}
      {#each ouvert.versions as v (v)}<span class="version">{v}</span>{/each}
      <span class="espace"></span>
      {#if surLancer && ouvert.sorte === "CronJob"}
        <button
          class="btn small primary"
          disabled={lancement}
          title={$trad("k8s.lancerHint")}
          onclick={() => lancer(ouvert.nom)}
        >{lancement ? $trad("k8s.lancement") : $trad("k8s.lancer")}</button>
      {/if}
      <span class="mesure">{formaterCpu(ouvert.cpu)}</span>
      <span class="mesure">{formaterRam(ouvert.ram)}</span>
    </div>

    {#if surSupprimer && enEchec.length > 1}
      <!-- **UN ECHEC CORRIGE LAISSE DES PODS DERRIERE LUI.** Les supprimer un par un demandait
           autant de confirmations qu'il y en avait : ici une seule, qui dit combien. -->
      <div class="menage">
        <button class="btn small danger" onclick={() => surSupprimer(enEchec)}>
          {$trad("k8s.supprimerLesEchecs", { n: enEchec.length })}
        </button>
      </div>
    {/if}

    <div class="pods">
      {#if ouvert.pods.length === 0}
        <!-- **« AUCUN POD » EST UNE REPONSE, PAS UN ECRAN VIDE.** C'est meme la reponse qu'on
             vient chercher sur un travail planifie qui n'a rien fait. -->
        <p class="rien">{$trad("k8s.aucunPodIci")}</p>
      {/if}
      {#each ouvert.pods as p (p.nom)}
        <K8sLignePod
          pod={p}
          choisi={choisi?.nom === p.nom}
          {maintenant}
          {kubectl}
          {surOuvrirPod}
          {surShell}
          surSupprimer={surSupprimer ? (pod) => surSupprimer([pod]) : undefined}
        />
      {/each}
    </div>
  </div>
{:else}
  <div class="objets">
    {#if elements.length === 0}
      <div class="vide">{vide}</div>
    {/if}
    {#each elements as e (e.sorte + e.nom)}
      {@const etat = etatDe(e)}
      <button class="objet" class:alerte={e.ennuyeux} onclick={() => (entre = e.nom)}>
        <span class="point {couleurDe(e)}"></span>
        <span class="nom">{e.nom}</span>
        {#if !e.declare}
          <!-- Ses pods existent, mais plus l'objet qui les a crees : on le dit au lieu de le
               cacher, sinon on cherche pourquoi il « manque » a la liste declaree. -->
          <span class="etiquette orphelin" title={$trad("k8s.orphelinAide")}>{$trad("k8s.orphelin")}</span>
        {/if}
        {#if e.planification}<span class="planif" title={$trad("k8s.planification")}>{e.planification}</span>{/if}
        <span class="etat" class:incomplet={etat.alerte}>{etat.texte}</span>
        {#each e.versions.slice(0, 2) as v (v)}<span class="version">{v}</span>{/each}
        {#if e.versions.length > 2}<span class="version">+{e.versions.length - 2}</span>{/if}
        <span class="espace"></span>
        <span class="pods-compte">{$trad("k8s.podsN", { n: e.pods.length })}</span>
        <!-- **PAS DE MESURE, PAS DE JAUGE.** Un cadre vide se lit « consommation nulle », alors
             qu'un travail planifie qui ne tourne pas n'a simplement rien a mesurer. -->
        {#if e.cpu !== null || e.ram !== null}
          <span class="jauge" title={$trad("k8s.cpu")}>
            <span class="remplissage" style="width:{Math.min(100, ((e.cpu ?? 0) / cpuMax) * 100)}%"></span>
            <span class="valeur">{formaterCpu(e.cpu)}</span>
          </span>
          <span class="jauge" title={$trad("k8s.ram")}>
            <span class="remplissage ram" style="width:{Math.min(100, ((e.ram ?? 0) / ramMax) * 100)}%"></span>
            <span class="valeur">{formaterRam(e.ram)}</span>
          </span>
        {:else}
          <span class="sans-mesure">—</span>
        {/if}
        <span class="entrer">›</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .objets, .detail-objet, .pods { display: flex; flex-direction: column; }
  .objets > *, .detail-objet > *, .pods > * { flex: none; }
  .objets { gap: 0.1rem; }
  .pods { gap: 0.05rem; }

  .objet {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    padding: 0.4rem 0.5rem;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    color: inherit;
    text-align: left;
    cursor: pointer;
    font-size: 0.82rem;
  }
  .objet:hover { border-color: var(--accent); background: var(--bg-tertiary); }
  .objet.alerte { border-left: 2px solid var(--error); }

  .point { width: 7px; height: 7px; border-radius: 50%; flex: none; }
  .point.bon { background: var(--success); }
  .point.mauvais { background: var(--error); }
  .point.eteint { background: var(--text-muted); }

  .nom {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 22rem;
  }
  .mono { font-family: var(--font-mono); font-size: 0.72rem; color: var(--text-muted); }
  .planif {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    padding: 0.05rem 0.3rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    white-space: nowrap;
  }
  .sans-mesure { color: var(--text-muted); font-size: 0.72rem; width: 9.4rem; text-align: right; }
  .etat { font-size: 0.74rem; color: var(--text-muted); white-space: nowrap; }
  .etat.incomplet, .incomplet { color: var(--warning); }
  .version {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    padding: 0.05rem 0.3rem;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
    color: var(--text-muted);
    white-space: nowrap;
  }
  .etiquette {
    font-size: 0.66rem;
    padding: 0.05rem 0.3rem;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }
  .etiquette.orphelin { color: var(--warning); }
  .espace { flex: 1; }
  .pods-compte { font-size: 0.72rem; color: var(--text-muted); white-space: nowrap; }

  .jauge {
    position: relative;
    width: 4.6rem;
    height: 1.1rem;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
    overflow: hidden;
    flex: none;
  }
  .remplissage { position: absolute; inset: 0 auto 0 0; background: color-mix(in srgb, var(--accent) 30%, transparent); }
  .remplissage.ram { background: color-mix(in srgb, var(--success) 26%, transparent); }
  .jauge .valeur {
    position: relative;
    display: block;
    text-align: right;
    padding-right: 0.3rem;
    line-height: 1.1rem;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    color: var(--text-secondary);
  }
  .entrer { color: var(--text-muted); font-size: 1rem; flex: none; }

  .fil { display: flex; align-items: center; gap: 0.6rem; padding: 0.2rem 0 0.5rem; }
  .retour {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0.2rem 0;
  }
  .fil-nom { font-weight: 600; font-size: 0.92rem; }
  .sorte { font-size: 0.7rem; color: var(--text-muted); }

  .faits {
    display: flex;
    align-items: center;
    gap: 0.8rem;
    flex-wrap: wrap;
    padding: 0.4rem 0.5rem;
    margin-bottom: 0.4rem;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
  }
  .fait { display: flex; align-items: baseline; gap: 0.35rem; font-size: 0.78rem; }
  .cle { color: var(--text-muted); font-size: 0.7rem; }
  .mesure { font-family: var(--font-mono); font-size: 0.74rem; color: var(--text-muted); }

  .menage { display: flex; justify-content: flex-end; padding-bottom: 0.3rem; }
  .menage .danger { color: var(--error); border-color: var(--error); }

  .rien, .vide { color: var(--text-muted); font-size: 0.82rem; padding: 1rem 0.5rem; text-align: center; }
</style>
