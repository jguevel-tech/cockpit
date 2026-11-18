<script lang="ts">
  /**
   * L'ecran des pods d'un namespace.
   *
   * **CE QUI FAIT LA DIFFERENCE AVEC L'ECRAN D'EN FACE : L'ORDRE, LES FAMILLES ET LE FLUX.** Un
   * namespace reel contient 298 pods pour 66 services, la plupart declines par marque. Tout est
   * affiche, mais range : des filtres en haut, les services d'une meme famille sous une seule
   * ligne, ce qui reclame une action d'abord. Et la liste se met a jour par un FLUX : on ne
   * recharge jamais les 6,3 Mo que pese sa lecture complete.
   */
  import { onMount, onDestroy } from "svelte";
  import { ecouter, invoke, type Detacher } from "../../coquille";
  import { activeTab, pendingTerminalCommand } from "../../stores/ui";
  import { notify } from "../../stores/toast";
  import { demanderConfirmation } from "../../stores/confirm";
  import { signalerErreur } from "../../stores/errors";
  import { getAppSettings, setAppSetting } from "../../api/recorder";
  import { trad } from "../../i18n";
  import PodJournal from "./PodJournal.svelte";
  import VueRessources from "./VueRessources.svelte";
  import K8sEnsemble from "./K8sEnsemble.svelte";
  import K8sObjets from "./K8sObjets.svelte";
  import K8sLignePod from "./K8sLignePod.svelte";
  import {
    k8sContextes, k8sNamespaces, k8sPods, k8sEvenements, k8sYaml,
    k8sKubectlPresent, k8sAjouterUnCluster, k8sSurveillanceLire, k8sSurveillanceEcrire,
    k8sHistorique, k8sWorkloads, k8sSupprimerUnPod, k8sLancerUneTache, type Contexte, type ReglagesSurveillance,
  } from "../../api/k8s";
  import {
    filtrer, grouper, formaterCpu, formaterRam, age, grouperLesNamespaces,
    appliquer, appliquerLesMesures, cibleDeDemarrage, enFamilles, comptesDesFiltres,
    appliquerLeFiltre, peutAllerA, unifier, deSorte, filtrerLesElements, ensemble,
    namespaceRetenu, retenir,
    type Pod, type Mesure, type Filtre, type Workload, type CibleRetenue,
  } from "../../k8s/vue";
  import {
    noter, depuisEnregistre, fusionner, type Historique,
  } from "../../k8s/mesures";

  let { name }: { name: string } = $props();

  let contextes: Contexte[] = $state([]);
  let contexte = $state("");
  let namespaces: string[] = $state([]);
  let namespace = $state("");
  let pods: Pod[] = $state([]);
  /// Ce que le namespace DECLARE. Lu une fois par ouverture d'ecran : le flux ne porte que les
  /// pods, et un travail planifie ne change pas trente fois par minute.
  let declares: Workload[] = $state([]);
  let sansMesures: string | null = $state(null);
  let recherche = $state("");
  let filtre: Filtre = $state("tout");
  let chargement = $state(false);
  let panne: string | null = $state(null);
  let enDirect = $state(false);
  let ouverts = $state(new Set<string>());
  let kubectl = $state(false);

  /// **CINQ VUES, ET CHACUNE REPOND A UNE QUESTION DIFFERENTE.** « Qu'est-ce qu'il y a
  /// la-dedans », « quels services », « quelles taches planifiees », « qu'est-ce qui tourne
  /// en ce moment », « qu'est-ce que ca consomme ». Une seule liste qui melangeait tout
  /// obligeait a deplier des centaines de pods pour lire soixante-dix-neuf noms.
  type Vue = "ensemble" | "services" | "taches" | "pods" | "ressources";
  let vue: Vue = $state("ensemble");
  /// L'historique des mesures, tenu par Cockpit : le cluster ne rend qu'un instantane.
  /// Ce que l'ecran mesure pendant qu'on le regarde.
  let direct: Historique = $state(new Map());
  /// Ce que la surveillance a enregistre avant qu'on arrive.
  let enregistre: Historique = $state(new Map());
  /// Ce que l'utilisateur a declare surveiller.
  let surveillance: ReglagesSurveillance = $state({ cibles: [], retention_heures: 24 });
  let fenetreSecondes = $state(300);
  let rafraichissement = $state(5);
  let focus: string | null = $state(null);

  let ouvert: "cluster" | "namespace" | null = $state(null);
  let chercheNamespace = $state("");
  let ajoutOuvert = $state(false);
  let texteColle = $state("");
  let ajoutEnCours = $state(false);

  /// Le pod dont on regarde le detail, et ce qu'on y regarde.
  let choisi: Pod | null = $state(null);
  let volet: "logs" | "evenements" | "yaml" = $state("logs");
  let contenu = $state("");
  let contenuCharge = $state(false);

  let maintenant = $state(Date.now());
  let horloge: ReturnType<typeof setInterval> | null = null;
  let detacheurs: Detacher[] = [];

  const cle = $derived(`k8s.cible.${name}`);
  /// Ce que le projet retient, tel qu'il a ete relu puis complete a chaque geste.
  let retenue: CibleRetenue = {};
  /// Ce qui est declare et ce qui tourne, rassemble. Un objet sans pod y figure, un pod dont
  /// le createur a disparu aussi : les deux existent, et les deux se cherchent.
  const elements = $derived(unifier(declares, grouper(pods)));
  const trouves = $derived(filtrerLesElements(elements, recherche));
  const services = $derived(deSorte(trouves, "services"));
  const taches = $derived(deSorte(trouves, "taches"));
  const resume = $derived(ensemble(elements, pods));
  const filtres = $derived(appliquerLeFiltre(pods, filtre));
  const visibles = $derived(filtrer(filtres, recherche));
  const familles = $derived(enFamilles(grouper(visibles)));
  /// Les pods en echec du namespace, recherche comprise : on ne supprime que ce qu'on
  /// pourrait voir. Le filtre des pods n'y entre pas, il ne vaut que pour leur vue.
  const enEchec = $derived(filtrer(pods, recherche).filter((p) => p.ennuyeux));
  /// **UNE RECHERCHE QUI NE TROUVE RIEN ICI PEUT TROUVER AILLEURS, ET LE TAIRE EST UN
  /// CUL-DE-SAC.** Chercher « snap » dans les pods ne rend rien quand la tache planifiee de ce
  /// nom ne s'est jamais declenchee : on le dit, et on y emmene.
  /// TypeScript reduit `vue` a sa valeur de depart tant que rien ne l'a reaffectee : sans ce
  /// detour, la comparaison ci-dessous est signalee comme toujours fausse.
  const vueCourante = $derived(vue as Vue);
  const ailleurs = $derived(
    recherche.trim().length === 0
      ? []
      : (
          [
            { vue: "services" as const, libelle: $trad("k8s.ongletServices"), n: services.length },
            { vue: "taches" as const, libelle: $trad("k8s.ongletTaches"), n: taches.length },
            { vue: "pods" as const, libelle: $trad("k8s.ongletPods"), n: visibles.length },
          ]
        ).filter((x) => x.n > 0 && x.vue !== vueCourante),
  );
  /// Ce que la vue affichee montre en ce moment, pour savoir si elle est vide.
  const combienIci = $derived(
    vueCourante === "services"
      ? services.length
      : vueCourante === "taches" ? taches.length : visibles.length,
  );
  const aVoir = $derived(elements.filter((e) => e.ennuyeux));
  const comptes = $derived(comptesDesFiltres(pods));
  const contexteActif = $derived(contextes.find((c) => c.nom === contexte));
  const famillesDeNamespaces = $derived(
    grouperLesNamespaces(
      namespaces.filter((n) => n.toLowerCase().includes(chercheNamespace.toLowerCase())),
    ),
  );
  /// La reference des jauges : le plus gros consommateur affiche. Sans reference commune, deux
  /// barres de meme longueur diraient deux choses differentes.
  const cpuMax = $derived(Math.max(1, ...familles.flatMap((f) => f.groupes.map((g) => g.cpu ?? 0))));
  const ramMax = $derived(Math.max(1, ...familles.flatMap((f) => f.groupes.map((g) => g.ram ?? 0))));
  /// Une recherche ou un filtre ouvre ce qu'il trouve : un resultat replie ne se montre pas.
  const toutOuvert = $derived(recherche.trim().length > 0 || filtre !== "tout");
  const historique = $derived(fusionner(enregistre, direct));
  /// La cible affichee est-elle surveillee en continu ?
  const surveillee = $derived(
    surveillance.cibles.find((c) => c.contexte === contexte && c.namespace === namespace) ?? null,
  );

  /// **LES FILTRES DE CETTE VUE NE PORTENT QUE SUR DES PODS.** « Services » et « taches
  /// planifiees » en sont partis : ils comptaient les pods d'un deploiement ou d'un travail
  /// sous le nom de l'objet, d'ou « taches planifiees 264 » sur un namespace qui en declare 79.
  /// Ces deux-la ont leur propre vue, qui compte des objets.
  const LIBELLES: { id: Filtre; libelle: Parameters<typeof $trad>[0] }[] = [
    { id: "tout", libelle: "k8s.filtreTout" },
    { id: "avoir", libelle: "k8s.filtreAvoir" },
    { id: "marche", libelle: "k8s.filtreMarche" },
    { id: "termines", libelle: "k8s.filtreTermines" },
  ];

  /// Au-dela, on compte au lieu de dessiner : cent pastilles ne se lisent plus.
  const PASTILLES_MAX = 10;

  onMount(() => {
    horloge = setInterval(() => (maintenant = Date.now()), 1000);
    void demarrer();
    return () => {
      if (horloge) clearInterval(horloge);
    };
  });

  onDestroy(() => {
    detacher();
    void invoke("k8s_arreter_le_suivi").catch(() => {});
  });

  function detacher() {
    for (const d of detacheurs) d();
    detacheurs = [];
    enDirect = false;
  }

  async function demarrer() {
    try {
      contextes = await k8sContextes();
      kubectl = await k8sKubectlPresent();
      surveillance = await k8sSurveillanceLire();
    } catch (e) {
      panne = String(e);
      return;
    }
    const reglages = await getAppSettings().catch(() => ({}) as Record<string, string>);
    let vise: CibleRetenue = {};
    try {
      vise = JSON.parse(reglages[cle] ?? "{}");
      if (typeof vise.fenetre === "number") fenetreSecondes = vise.fenetre;
      if (typeof vise.rythme === "number") rafraichissement = vise.rythme;
    } catch {
      // Un reglage illisible ne doit pas empecher d'ouvrir l'ecran : on repart du defaut.
      vise = {};
    }
    retenue = vise;
    const servables = contextes.filter((c) => !c.obstacle);
    const depart =
      servables.find((c) => c.nom === vise.contexte) ??
      servables.find((c) => c.courant) ??
      servables[0];
    if (!depart) return;
    await choisirLeCluster(depart.nom, namespaceRetenu(vise, depart.nom), false);
  }

  /**
   * `enregistrer` distingue le GESTE de la restauration.
   *
   * **UNE RESTAURATION N'ECRIT JAMAIS.** Sinon, le jour ou le namespace retenu ne se retrouve
   * pas dans la liste, l'ecran remplace le choix de l'utilisateur par son defaut et l'efface
   * pour de bon. C'est ce qui a ete signale : un projet revenait sur le namespace du contexte.
   */
  async function choisirLeCluster(
    nom: string,
    namespaceVise: string | null = null,
    enregistrer = true,
  ) {
    // Un cluster choisi a la main revient sur SON dernier namespace, pas sur celui du contexte.
    namespaceVise ??= namespaceRetenu(retenue, nom);
    contexte = nom;
    ouvert = null;
    pods = [];
    panne = null;
    detacher();
    chargement = true;
    try {
      namespaces = await k8sNamespaces(nom);
    } catch (e) {
      panne = String(e);
      chargement = false;
      return;
    }
    const duContexte = contextes.find((c) => c.nom === nom)?.namespace ?? null;
    await choisirLeNamespace(cibleDeDemarrage(namespaces, namespaceVise, duContexte), enregistrer);
  }

  async function choisirLeNamespace(nom: string, enregistrer = true) {
    namespace = nom;
    ouvert = null;
    chercheNamespace = "";
    choisi = null;
    declares = [];
    ouverts = new Set();
    direct = new Map();
    enregistre = new Map();
    focus = null;
    detacher();
    if (enregistrer) {
      enregistrerLaCible();
    }
    await charger();
  }

  /// Ce qu'on retient d'un projet : ou l'on regarde, et comment.
  function enregistrerLaCible() {
    retenue = { ...retenir(retenue, contexte, namespace), fenetre: fenetreSecondes, rythme: rafraichissement };
    void setAppSetting(cle, JSON.stringify(retenue)).catch((e) => signalerErreur("k8s.reglage", String(e)));
  }

  /// Lit la liste ENTIERE, puis passe le relais au flux. C'est la seule lecture complete.
  async function charger() {
    if (!contexte || !namespace) return;
    chargement = true;
    panne = null;
    try {
      // **CE QUI A ETE ENREGISTRE ARRIVE AVANT LE DIRECT.** Sans ca, la courbe repart de zero
      // a chaque ouverture, alors que la surveillance a peut-etre mesure toute la journee.
      void relireLHistorique();
      // **CE QUI EST DECLARE NE BLOQUE PAS CE QUI TOURNE.** Les droits se donnent ressource
      // par ressource : un refus sur les `cronjobs` ne doit pas priver de la liste des pods.
      void k8sWorkloads(contexte, namespace)
        .then((w) => (declares = w))
        .catch((e) => signalerErreur("k8s.workloads", String(e)));
      const lu = await k8sPods(contexte, namespace);
      pods = lu.pods;
      sansMesures = lu.sans_mesures;
      suivre(lu.version);
    } catch (e) {
      panne = String(e);
      pods = [];
      declares = [];
    } finally {
      chargement = false;
    }
  }

  async function relireLHistorique() {
    if (!contexte || !namespace) return;
    try {
      const depuis = Date.now() - surveillance.retention_heures * 3_600_000;
      enregistre = depuisEnregistre(await k8sHistorique(contexte, namespace, depuis));
    } catch (e) {
      // Un historique illisible ne doit pas empecher de voir le direct.
      signalerErreur("k8s.historique", String(e));
    }
  }

  /// Active ou coupe la surveillance du namespace affiche, avec le rythme demande.
  ///
  /// **C'EST LE MEME REGLAGE QUE DANS LES PARAMETRES**, pas un second : il n'y a qu'un endroit
  /// ou la verite est rangee, et deux facons de l'atteindre.
  async function reglerLaSurveillance(actif: boolean, periode: number) {
    const autres = surveillance.cibles.filter(
      (c) => !(c.contexte === contexte && c.namespace === namespace),
    );
    const cibles = actif
      ? [...autres, { contexte, namespace, periode, actif: true }]
      : autres;
    try {
      surveillance = await k8sSurveillanceEcrire({ ...surveillance, cibles });
      if (actif) await relireLHistorique();
    } catch (e) {
      notify(String(e));
    }
  }

  /// Branche le flux a partir de la version de la liste qu'on vient de lire : reprendre
  /// ailleurs laisserait passer en silence ce qui a change entre les deux.
  function suivre(version: string) {
    detacher();
    detacheurs = [
      ecouter<{ sorte: string; pod: Pod }>("k8s_changement", (e) => {
        pods = appliquer(pods, e.payload.sorte, e.payload.pod);
        if (choisi && e.payload.pod.nom === choisi.nom) choisi = e.payload.pod;
      }),
      ecouter<Mesure[]>("k8s_mesures", (e) => {
        pods = appliquerLesMesures(pods, e.payload);
        direct = noter(direct, e.payload, Date.now());
      }),
      // Notre point de reprise n'est plus valable : le cluster nous le dit, on relit.
      ecouter<null>("k8s_relire", () => void charger()),
      ecouter<string>("k8s_panne", (e) => {
        enDirect = false;
        panne = e.payload;
      }),
    ];
    void invoke("k8s_periode_des_mesures", { secondes: rafraichissement }).catch(() => {});
    void invoke("k8s_suivre", { contexte, namespace, version })
      .then(() => (enDirect = true))
      .catch((e) => {
        enDirect = false;
        signalerErreur("k8s.suivi", String(e));
      });
  }

  function basculer(quoi: string) {
    const suite = new Set(ouverts);
    if (suite.has(quoi)) suite.delete(quoi);
    else suite.add(quoi);
    ouverts = suite;
  }

  async function ouvrirLeDetail(pod: Pod, lequel: "logs" | "evenements" | "yaml" = "logs") {
    choisi = pod;
    volet = lequel;
    if (lequel !== "logs") await lireLeDetail();
  }

  async function lireLeDetail() {
    if (!choisi || volet === "logs") return;
    contenuCharge = false;
    contenu = "";
    try {
      if (volet === "yaml") {
        contenu = await k8sYaml(contexte, namespace, choisi.nom);
      } else {
        const liste = await k8sEvenements(contexte, namespace, choisi.nom);
        contenu = liste.length === 0
          ? $trad("k8s.aucunEvenement")
          : liste.map(formaterEvenement).join("\n\n");
      }
    } catch (e) {
      contenu = String(e);
    } finally {
      contenuCharge = true;
    }
  }

  function formaterEvenement(e: Record<string, unknown>): string {
    const quand = String(e.lastTimestamp ?? e.eventTime ?? "");
    const heure = quand ? new Date(quand).toLocaleString() : "";
    const compte = Number(e.count ?? 1);
    const fois = compte > 1 ? ` (${$trad("k8s.foisN", { n: compte })})` : "";
    return `${heure}  ${e.type ?? ""}  ${e.reason ?? ""}${fois}\n${e.message ?? ""}`;
  }

  /// Ouvre un shell DANS le conteneur, par un vrai terminal Cockpit : on y retrouve ses
  /// volets, son historique et son copier-coller. Seul ce bouton demande `kubectl`.
  function ouvrirUnShell(pod: Pod) {
    const c = pod.noms_conteneurs[0];
    const ou = c ? ` -c ${c}` : "";
    const commande =
      `kubectl --context ${contexte} -n ${namespace} exec -it ${pod.nom}${ou}` +
      ` -- sh -c '[ -x /bin/bash ] && exec bash || exec sh'`;
    pendingTerminalCommand.set({ project: name, command: commande });
    activeTab.set("terminal");
  }

  async function ajouterUnCluster() {
    const texte = texteColle.trim();
    if (!texte) return;
    ajoutEnCours = true;
    try {
      const quoi = await k8sAjouterUnCluster(texte);
      // **ON EFFACE CE QU'ON VIENT DE COLLER** : un jeton d'acces n'a pas a rester affiche.
      texteColle = "";
      ajoutOuvert = false;
      contextes = await k8sContextes();
      const messages: string[] = [];
      if (quoi.renommes.length > 0) messages.push($trad("k8s.ajoutRenomme", { noms: quoi.renommes.join(", ") }));
      if (quoi.renouveles.length > 0) messages.push($trad("k8s.ajoutRenouvele", { noms: quoi.renouveles.join(", ") }));
      if (quoi.deja_la.length > 0) messages.push($trad("k8s.ajoutDejaLa", { noms: quoi.deja_la.join(", ") }));
      if (messages.length === 0) messages.push($trad("k8s.ajoutFait", { noms: quoi.ajoutes.join(", ") }));
      notify(messages.join(" "));
      const neuf = quoi.ajoutes[0];
      if (neuf && quoi.deja_la.length === 0) await choisirLeCluster(neuf);
    } catch (e) {
      notify(String(e));
    } finally {
      ajoutEnCours = false;
    }
  }

  /// Un fichier depose vaut un collage : c'est celui que l'interface web fait telecharger.
  async function surDepot(e: DragEvent) {
    e.preventDefault();
    const fichier = e.dataTransfer?.files?.[0];
    if (!fichier) return;
    try {
      texteColle = await fichier.text();
    } catch (err) {
      notify(String(err));
    }
  }

  function reglerLaFenetre(secondes: number) {
    fenetreSecondes = secondes;
    enregistrerLaCible();
  }

  function reglerLeRafraichissement(secondes: number) {
    rafraichissement = secondes;
    enregistrerLaCible();
    // Le flux n'est pas coupe : la courbe garde son historique quand on change de rythme.
    void invoke("k8s_periode_des_mesures", { secondes }).catch((e) =>
      signalerErreur("k8s.periode", String(e)),
    );
  }

  /**
   * Supprime des pods, apres confirmation NOMMEE.
   *
   * **ON NE DETRUIT RIEN SANS DIRE QUOI.** La fenetre cite le pod, ou le nombre quand il y en
   * a plusieurs : « supprimer ? » sans nom ne permet pas de repondre. Et on ne touche jamais a
   * ce qui les a crees : un pod tenu par un deploiement ou un travail est RECREE par lui,
   * c'est le geste courant pour repartir sur un pod casse.
   *
   * Le cluster tranche : un refus de droits remonte tel quel, pod par pod, parce qu'on ne sait
   * pas a l'avance ce qu'il permet.
   */
  /// Lance une tache planifiee maintenant.
  ///
  /// **CA AGIT SUR LE CLUSTER, SOUVENT EN PRODUCTION** : la confirmation cite la tache, le
  /// cluster et le namespace, pour qu'on ne lance pas sur la prod ce qu'on croyait lancer en
  /// qualification. Le pod arrive ensuite par le flux, sous la tache.
  async function lancerUneTache(tache: string) {
    const message = $trad("k8s.lancerConfirmation", { nom: tache, contexte, namespace });
    if (!(await demanderConfirmation({ message, action: $trad("k8s.lancer") }))) return;
    try {
      const travail = await k8sLancerUneTache(contexte, namespace, tache);
      notify($trad("k8s.lancerFait", { nom: travail }), "info", 5000, { report: false });
    } catch (e) {
      notify($trad("k8s.lancerEchec", { nom: tache, erreur: String(e) }));
    }
  }

  async function supprimerDesPods(cibles: Pod[], message?: string) {
    if (cibles.length === 0) return;
    message ??=
      cibles.length === 1
        ? $trad("k8s.supprimerConfirmation", { nom: cibles[0].nom })
        : $trad("k8s.supprimerConfirmationN", { n: cibles.length });
    if (!(await demanderConfirmation({ message, action: $trad("k8s.supprimer") }))) return;

    // **PLUSIEURS A LA FOIS, MAIS PAS TOUS.** Un namespace peut laisser des centaines de pods
    // en echec : un par un, le nettoyage durait des minutes. Six demandes en cours au plus,
    // pour ne pas envoyer d'un coup des centaines de requetes au cluster.
    const reste = [...cibles];
    let faits = 0;
    let refus = false;
    const ouvrier = async () => {
      while (!refus) {
        const p = reste.shift();
        if (!p) return;
        try {
          await k8sSupprimerUnPod(contexte, namespace, p.nom);
          faits += 1;
        } catch (e) {
          // On s'arrete au premier refus : les suivants echoueraient de la meme facon, et une
          // pluie de messages identiques n'apprend rien de plus.
          if (!refus) notify($trad("k8s.supprimerEchec", { nom: p.nom, erreur: String(e) }));
          refus = true;
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(6, cibles.length) }, ouvrier));
    if (faits > 0) {
      notify($trad("k8s.supprimerFait", { n: faits }));
      // Le flux annonce leur disparition de lui-meme ; si le pod choisi vient de partir, on
      // ferme son detail pour ne pas laisser un ecran qui parle d'un pod qui n'est plus la.
      if (choisi && cibles.some((p) => p.nom === choisi?.nom)) choisi = null;
    }
  }

  function couleurDe(pod: Pod): string {
    if (pod.ennuyeux) return "mauvais";
    if (pod.etat === "Succeeded") return "fini";
    if (pod.etat === "Running" && pod.prets === pod.conteneurs) return "bon";
    return "attente";
  }
</script>

<div class="k8s">
  <div class="barre">
    <div class="cibles">
      <button
        class="pilule"
        class:actif={ouvert === "cluster"}
        onclick={() => (ouvert = ouvert === "cluster" ? null : "cluster")}
        disabled={contextes.length === 0}
        title={contexteActif?.serveur ?? ""}
      >
        <span class="pilule-cle">{$trad("k8s.cluster")}</span>
        <span class="pilule-valeur">{contexte || $trad("k8s.aucun")}</span>
        <span class="chevron">▾</span>
      </button>
      <button
        class="pilule"
        class:actif={ouvert === "namespace"}
        onclick={() => (ouvert = ouvert === "namespace" ? null : "namespace")}
        disabled={namespaces.length === 0 && !namespace}
      >
        <span class="pilule-cle">{$trad("k8s.namespace")}</span>
        <span class="pilule-valeur">{namespace || "—"}</span>
        <span class="chevron">▾</span>
      </button>
    </div>

    <input
      class="input recherche"
      type="search"
      bind:value={recherche}
      placeholder={$trad("k8s.chercher")}
      disabled={pods.length === 0}
    />

    <div class="vivant">
      <span class="direct" class:actif={enDirect} title={enDirect ? $trad("k8s.directAide") : ""}>
        {enDirect ? $trad("k8s.direct") : $trad("k8s.arrete")}
      </span>
      <button class="btn small ghost" onclick={() => void charger()} disabled={chargement}>
        {$trad("k8s.relire")}
      </button>
    </div>
  </div>

  {#if namespace}
    <!-- **CHAQUE ONGLET PORTE SON COMPTE, ET IL COMPTE CE QU'IL AFFICHE.** Celui des taches
         planifiees annonce les taches DECLAREES, pas les pods qu'elles ont laisses. -->
    <div class="vues">
      {#each [
        ["ensemble", $trad("k8s.ongletEnsemble"), -1],
        ["services", $trad("k8s.ongletServices"), resume.services],
        ["taches", $trad("k8s.ongletTaches"), resume.taches],
        ["pods", $trad("k8s.ongletPods"), resume.pods],
        ["ressources", $trad("k8s.ongletRessources"), -1],
      ] as [id, libelle, n] (id)}
        <button
          class="vue"
          class:actif={vue === id}
          onclick={() => (vue = id as Vue)}
        >
          {libelle}{#if (n as number) >= 0}<span class="compte-vue">{n}</span>{/if}
        </button>
      {/each}
      <span class="espace-vues"></span>
      {#if enEchec.length > 0}
        <!-- **LE MENAGE D'UN NAMESPACE SE FAIT EN UN GESTE.** Entrer dans chaque tache pour
             nettoyer ses echecs demandait autant de visites qu'il y a de taches qui ont rate. -->
        <button
          class="btn small danger tout-nettoyer"
          title={$trad("k8s.supprimerTousAide")}
          onclick={() => void supprimerDesPods(
            enEchec,
            $trad("k8s.supprimerTousConfirmation", { n: enEchec.length, ns: namespace }),
          )}
        >
          {$trad("k8s.supprimerLesEchecs", { n: enEchec.length })}
        </button>
      {/if}
    </div>
  {/if}

  {#if ailleurs.length > 0 && combienIci === 0 && vue !== "ensemble" && vue !== "ressources"}
    <div class="passerelle">
      <span>{$trad("k8s.rienIci")}</span>
      {#each ailleurs as a (a.vue)}
        <button class="lien" onclick={() => (vue = a.vue)}>
          {$trad("k8s.trouveDansN", { n: a.n, vue: a.libelle })}
        </button>
      {/each}
    </div>
  {/if}

  {#if pods.length > 0 && vue === "pods"}
    <div class="filtres">
      {#each LIBELLES as f (f.id)}
        {@const n = comptes[f.id]}
        <button
          class="filtre {f.id}"
          class:actif={filtre === f.id}
          disabled={n === 0 && f.id !== "tout"}
          onclick={() => (filtre = f.id)}
        >
          {$trad(f.libelle)}<span class="compte">{n}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if ouvert === "cluster"}
    <div class="panneau">
      {#each contextes as c (c.nom)}
        <button
          class="entree"
          class:choisie={c.nom === contexte}
          disabled={!!c.obstacle}
          onclick={() => void choisirLeCluster(c.nom)}
          title={c.obstacle ?? c.serveur}
        >
          <span class="entree-nom">{c.nom}</span>
          <span class="entree-detail">{c.obstacle ?? c.serveur}</span>
        </button>
      {/each}

      {#if !ajoutOuvert}
        <button class="btn small ajouter" onclick={() => (ajoutOuvert = true)}>
          {$trad("k8s.ajouter")}
        </button>
      {:else}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="ajout" ondrop={surDepot} ondragover={(e) => e.preventDefault()}>
          <ol class="etapes">
            <li>{$trad("k8s.ajouterEtape1")}</li>
            <li>{$trad("k8s.ajouterEtape2")}</li>
            <li>{$trad("k8s.ajouterEtape3")}</li>
          </ol>
          <textarea
            class="input colle"
            bind:value={texteColle}
            placeholder={$trad("k8s.ajouterZone")}
            spellcheck="false"
          ></textarea>
          <div class="ajout-bas">
            <button
              class="btn small primary"
              onclick={() => void ajouterUnCluster()}
              disabled={ajoutEnCours || !texteColle.trim()}
            >{$trad("k8s.ajouterBouton")}</button>
            <button class="btn small ghost" onclick={() => { ajoutOuvert = false; texteColle = ""; }}>
              {$trad("k8s.annuler")}
            </button>
            <span class="note">{$trad("k8s.ajouterNote")}</span>
          </div>
        </div>
      {/if}
    </div>
  {:else if ouvert === "namespace"}
    <div class="panneau">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input"
        bind:value={chercheNamespace}
        placeholder={$trad("k8s.chercherNamespace")}
        autofocus
      />
      {#if peutAllerA(chercheNamespace, namespaces)}
        <!-- **UN NAMESPACE ABSENT DE LA LISTE RESTE ATTEIGNABLE.** La liste vient des droits, et
             un cluster peut refuser de l'etablir tout en donnant acces aux pods. -->
        <button class="btn small primary aller" onclick={() => void choisirLeNamespace(chercheNamespace.trim())}>
          {$trad("k8s.allerA", { nom: chercheNamespace.trim() })}
        </button>
      {/if}
      {#if namespaces.length <= 1}
        <p class="note">{$trad("k8s.listeIncomplete")}</p>
      {/if}
      <div class="namespaces">
        {#each famillesDeNamespaces as famille (famille.famille)}
          {#if famille.famille}<div class="famille-titre">{famille.famille}</div>{/if}
          {#each famille.noms as n (n)}
            <button
              class="entree"
              class:choisie={n === namespace}
              onclick={() => void choisirLeNamespace(n)}
            >
              <span class="entree-nom">{n}</span>
            </button>
          {/each}
        {/each}
      </div>
    </div>
  {/if}

  <!-- **UNE PANNE PASSE AVANT « IL N'Y A RIEN ».** Dans l'autre ordre, un backend qui refuse de
       repondre affichait « aucun cluster » : l'ecran envoyait chercher un kubeconfig correct au
       lieu de dire ce qui s'etait passe. -->
  {#if panne}
    <div class="vide">
      <p class="erreur">{panne}</p>
      <button class="btn" onclick={() => void charger()}>{$trad("k8s.reessayer")}</button>
    </div>
  {:else if contextes.length === 0}
    <div class="vide">
      <p>{$trad("k8s.aucunKubeconfig")}</p>
      <p class="aide">{$trad("k8s.aucunKubeconfigAide")}</p>
    </div>
  {:else}
    <div class="corps">
      {#if vue === "ensemble"}
        <div class="page">
          <K8sEnsemble
            ensemble={resume}
            {aVoir}
            {maintenant}
            surVue={(v) => (vue = v)}
          />
        </div>
      {:else if vue === "services" || vue === "taches"}
        <div class="page">
          <!-- Changer de vue REMONTE la liste : sans cette cle, Svelte garderait l'instance et
               l'objet ouvert dans les taches resterait ouvert en passant aux services. -->
          {#key vue}
          <K8sObjets
            elements={vue === "services" ? services : taches}
            {maintenant}
            {kubectl}
            {choisi}
            surOuvrirPod={(pod, volet) => void ouvrirLeDetail(pod, volet)}
            surShell={ouvrirUnShell}
            surSupprimer={(cibles) => void supprimerDesPods(cibles)}
            surLancer={lancerUneTache}
            vide={recherche
              ? $trad("k8s.aucunResultat")
              : vue === "services" ? $trad("k8s.aucunService") : $trad("k8s.aucuneTache")}
          />
          {/key}
        </div>
      {:else if vue === "ressources"}
        <VueRessources
          {pods}
          {historique}
          {surveillee}
          retentionHeures={surveillance.retention_heures}
          surSurveillance={reglerLaSurveillance}
          {fenetreSecondes}
          {rafraichissement}
          {focus}
          surFenetre={reglerLaFenetre}
          surRafraichissement={reglerLeRafraichissement}
          surFocus={(nom) => (focus = nom)}
        />
      {:else}
      <div class="liste">
        {#if sansMesures}
          <p class="avis" title={sansMesures}>{$trad("k8s.sansMesures")}</p>
        {/if}
        {#if chargement && pods.length === 0}
          <div class="vide">{$trad("k8s.chargement")}</div>
        {:else if familles.length === 0}
          <div class="vide">{recherche ? $trad("k8s.aucunResultat") : $trad("k8s.aucunPod")}</div>
        {/if}

        {#each familles as famille (famille.sorte + famille.nom + famille.groupes[0].nom)}
          {@const cleFamille = `f:${famille.sorte}:${famille.nom}`}
          {@const familleOuverte = toutOuvert || ouverts.has(cleFamille) || !famille.nom}

          {#if famille.nom}
            <button
              class="famille"
              class:alerte={famille.ennuyeux}
              onclick={() => basculer(cleFamille)}
            >
              <span class="chevron">{familleOuverte ? "▾" : "▸"}</span>
              <span class="famille-nom">{famille.nom}</span>
              <span class="etiquette">{$trad("k8s.servicesN", { n: famille.groupes.length })}</span>
              <span class="sante">{famille.prets}/{famille.attendus}</span>
              <span class="espace"></span>
              <span class="mesure">{formaterCpu(famille.cpu)}</span>
              <span class="mesure">{formaterRam(famille.ram)}</span>
            </button>
          {/if}

          {#if familleOuverte}
            {#each famille.groupes as g (g.sorte + g.nom)}
              {@const cleGroupe = `g:${g.sorte}:${g.nom}`}
              {@const deplie = toutOuvert || ouverts.has(cleGroupe)}
              <article class="carte" class:alerte={g.ennuyeux} class:dans-famille={!!famille.nom}>
                <button class="tete" onclick={() => basculer(cleGroupe)}>
                  <span class="chevron">{deplie ? "▾" : "▸"}</span>

                  <span class="pastilles">
                    {#each g.pods.slice(0, PASTILLES_MAX) as p (p.nom)}
                      <span class="pastille {couleurDe(p)}" title={p.etat}></span>
                    {/each}
                    {#if g.pods.length > PASTILLES_MAX}
                      <span class="reste">+{g.pods.length - PASTILLES_MAX}</span>
                    {/if}
                  </span>

                  <span class="nom">{g.nom}</span>
                  {#if g.sorte}<span class="sorte">{g.sorte}</span>{/if}
                  <span class="sante" class:incomplet={g.prets < g.attendus}>
                    {g.prets}/{g.attendus}
                  </span>
                  {#each g.versions.slice(0, 2) as v (v)}
                    <span class="version">{v}</span>
                  {/each}
                  {#if g.versions.length > 2}
                    <span class="version">+{g.versions.length - 2}</span>
                  {/if}

                  <span class="espace"></span>

                  <span class="jauge" title={$trad("k8s.cpu")}>
                    <span class="remplissage" style="width:{Math.min(100, ((g.cpu ?? 0) / cpuMax) * 100)}%"></span>
                    <span class="valeur">{formaterCpu(g.cpu)}</span>
                  </span>
                  <span class="jauge" title={$trad("k8s.ram")}>
                    <span class="remplissage ram" style="width:{Math.min(100, ((g.ram ?? 0) / ramMax) * 100)}%"></span>
                    <span class="valeur">{formaterRam(g.ram)}</span>
                  </span>
                </button>

                {#if deplie}
                  <div class="pods">
                    {#each g.pods as p (p.nom)}
                      <K8sLignePod
                        pod={p}
                        choisi={choisi?.nom === p.nom}
                        {maintenant}
                        {kubectl}
                        surOuvrirPod={(pod, volet) => void ouvrirLeDetail(pod, volet)}
                        surShell={ouvrirUnShell}
                        surSupprimer={(pod) => void supprimerDesPods([pod])}
                      />
                    {/each}
                  </div>
                {/if}
              </article>
            {/each}
          {/if}
        {/each}
      </div>
      {/if}

      {#if choisi && vue !== "ensemble" && vue !== "ressources"}
        <aside class="detail">
          <div class="detail-tete">
            <div class="detail-titre">
              <span class="pastille {couleurDe(choisi)}"></span>
              <span class="detail-nom">{choisi.nom}</span>
            </div>
            <button class="btn small ghost" onclick={() => (choisi = null)} aria-label={$trad("k8s.fermer")}>✕</button>
          </div>

          <div class="detail-faits">
            <span>{choisi.etat}</span>
            <span>{age(choisi.depuis, maintenant)}</span>
            {#if choisi.version}<span class="version">{choisi.version}</span>{/if}
            {#if choisi.machine}<span title={$trad("k8s.machine")}>⌗ {choisi.machine}</span>{/if}
            {#if choisi.redemarrages > 0}<span class="redemarrages">⟳ {choisi.redemarrages}</span>{/if}
          </div>

          <div class="onglets">
            {#each [["logs", $trad("k8s.logs")], ["evenements", $trad("k8s.evenements")], ["yaml", $trad("k8s.yaml")]] as [id, libelle] (id)}
              <button
                class="onglet"
                class:actif={volet === id}
                onclick={() => { volet = id as typeof volet; void lireLeDetail(); }}
              >{libelle}</button>
            {/each}
          </div>

          {#if volet === "logs"}
            <!-- Changer de pod remonte un journal NEUF : sans cette cle, les lignes du pod
                 precedent resteraient a l'ecran sous un autre nom. -->
            {#key choisi.nom}
              <PodJournal
                {contexte}
                {namespace}
                pod={choisi.nom}
                conteneurs={choisi.noms_conteneurs}
                redemarrages={choisi.redemarrages}
              />
            {/key}
          {:else}
            <pre class="contenu">{contenuCharge ? contenu : $trad("k8s.chargement")}</pre>
          {/if}
        </aside>
      {/if}
    </div>
  {/if}
</div>

<style>
  .k8s {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    gap: 0.55rem;
  }
  /* **RIEN NE SE COMPRIME DANS CETTE COLONNE, SAUF LE CORPS.** Sans cette regle, une rangee de
     trente pixels est ecrasee jusqu'a disparaitre quand la place manque : la barre d'onglets
     ne s'affichait plus du tout, et rien ne le signalait. */
  .k8s > * { flex: none; }
  .k8s > .corps { flex: 1; }

  /* ── La barre du haut ─────────────────────────────────────────────────────── */
  .barre { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; }
  .cibles { display: flex; gap: 0.4rem; }

  .pilule {
    display: flex;
    align-items: baseline;
    gap: 0.45rem;
    padding: 0.38rem 0.7rem;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 999px;
    color: var(--text-primary);
    cursor: pointer;
    font-size: 0.85rem;
    transition: border-color 0.12s ease, background 0.12s ease;
  }
  .pilule:hover:not(:disabled) { border-color: var(--border-strong); }
  .pilule.actif { border-color: var(--accent); background: var(--accent-soft); }
  .pilule:disabled { opacity: 0.5; cursor: not-allowed; }
  .pilule-cle { color: var(--text-muted); font-size: 0.68rem; text-transform: uppercase; letter-spacing: 0.04em; }
  .pilule-valeur { font-weight: 600; }
  .chevron { color: var(--text-muted); font-size: 0.68rem; flex: none; }

  .recherche { flex: 1; min-width: 11rem; border-radius: 999px; }
  .vivant { display: flex; align-items: center; gap: 0.5rem; }
  .direct { color: var(--text-muted); font-size: 0.76rem; }
  .direct.actif { color: var(--success); }
  .direct.actif::before { content: "● "; }

  .vues { display: flex; align-items: center; gap: 0.2rem; border-bottom: 1px solid var(--border-color); }
  .espace-vues { flex: 1; }
  .tout-nettoyer { margin-bottom: 0.3rem; }
  .vue {
    padding: 0.3rem 0.8rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 0.84rem;
  }
  .vue:hover { color: var(--text-primary); }
  .vue.actif { color: var(--text-primary); border-bottom-color: var(--accent); }

  /* ── Les filtres ──────────────────────────────────────────────────────────── */
  .filtres { display: flex; gap: 0.35rem; flex-wrap: wrap; }
  .filtre {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.26rem 0.7rem;
    background: transparent;
    border: 1px solid var(--border-color);
    border-radius: 999px;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 0.78rem;
    transition: color 0.12s ease, border-color 0.12s ease, background 0.12s ease;
  }
  .filtre:hover:not(:disabled) { color: var(--text-primary); border-color: var(--border-strong); }
  .filtre:disabled { opacity: 0.4; cursor: default; }
  .filtre .compte {
    padding: 0.02rem 0.4rem;
    border-radius: 999px;
    background: var(--bg-tertiary);
    color: var(--text-muted);
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
  }
  .filtre.actif { color: var(--text-primary); border-color: var(--accent); background: var(--accent-soft); }
  .filtre.actif .compte { background: var(--accent); color: #fff; }
  .filtre.avoir.actif { border-color: var(--error); background: var(--error-soft); }
  .filtre.avoir.actif .compte { background: var(--error); }

  /* ── Les panneaux de choix ────────────────────────────────────────────────── */
  .panneau {
    background: var(--surface-base);
    border: 1px solid var(--border-color);
    border-radius: var(--radius);
    padding: 0.5rem;
    max-height: 24rem;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.25);
  }
  .panneau > * { flex: none; }
  .namespaces { display: flex; flex-direction: column; gap: 0.08rem; margin-top: 0.4rem; }
  .famille-titre {
    color: var(--text-muted);
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 0.55rem 0.45rem 0.15rem;
  }
  .entree {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 1rem;
    width: 100%;
    padding: 0.35rem 0.5rem;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    text-align: left;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .entree:hover:not(:disabled) { background: var(--bg-tertiary); }
  .entree.choisie { border-color: var(--accent); background: var(--accent-soft); }
  .entree:disabled { opacity: 0.55; cursor: not-allowed; }
  .entree-detail { color: var(--text-muted); font-size: 0.74rem; }

  .ajouter { align-self: flex-start; margin-top: 0.3rem; }
  .aller { align-self: flex-start; margin-top: 0.35rem; }
  .ajout { display: flex; flex-direction: column; gap: 0.5rem; padding: 0.5rem 0.2rem 0.2rem; }
  .etapes { margin: 0; padding-left: 1.1rem; color: var(--text-secondary); font-size: 0.8rem; line-height: 1.6; }
  .colle { min-height: 7rem; font-family: var(--font-mono); font-size: 0.74rem; resize: vertical; }
  .ajout-bas { display: flex; gap: 0.35rem; align-items: center; flex-wrap: wrap; }
  .note { color: var(--text-muted); font-size: 0.74rem; }

  /* ── La liste ─────────────────────────────────────────────────────────────── */
  .corps { display: flex; gap: 0.7rem; flex: 1; min-height: 0; }
  /* Une vue qui n'est pas une liste infinie : elle defile, et ses enfants ne s'ecrasent pas. */
  .page {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: auto;
    padding: 0.5rem 0.2rem 1rem;
  }
  .page > * { flex: none; }

  .passerelle {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    padding: 0.4rem 0.5rem;
    margin: 0.3rem 0;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    font-size: 0.8rem;
    color: var(--text-muted);
  }
  .passerelle .lien {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 0.8rem;
    padding: 0;
  }

  .compte-vue {
    margin-left: 0.35rem;
    padding: 0.02rem 0.28rem;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
    color: var(--text-muted);
    font-size: 0.68rem;
  }

  .liste {
    flex: 1;
    min-width: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    padding-right: 0.15rem;
  }
  /* **DANS UNE COLONNE FLEX, LES ENFANTS S'ECRASENT AU LIEU DE DEBORDER.** Avec 78 groupes
     dans une hauteur fixe, chacun se retrouvait haut de douze pixels : des bandes vides, sans
     une lettre lisible. `overflow: auto` ne suffit pas, c'est `flex-shrink` qu'il faut couper. */
  .liste > * { flex: none; }

  .avis {
    color: var(--text-muted);
    font-size: 0.78rem;
    padding: 0.35rem 0.6rem;
    margin: 0;
    border: 1px dashed var(--border-color);
    border-radius: var(--radius-sm);
  }

  .famille {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    width: 100%;
    padding: 0.4rem 0.6rem;
    margin-top: 0.25rem;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border-color);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 0.8rem;
    text-align: left;
  }
  .famille:hover { color: var(--text-primary); }
  .famille-nom { font-weight: 600; }
  .famille.alerte .famille-nom { color: var(--error); }
  .etiquette {
    padding: 0.03rem 0.45rem;
    border-radius: 999px;
    background: var(--bg-tertiary);
    color: var(--text-muted);
    font-size: 0.7rem;
  }

  .carte {
    border: 1px solid var(--border-color);
    border-radius: var(--radius);
    background: var(--bg-secondary);
    overflow: hidden;
    transition: border-color 0.12s ease;
  }
  .carte:hover { border-color: var(--border-strong); }
  .carte.alerte { border-color: var(--error); }
  .carte.dans-famille { margin-left: 1.1rem; }

  .tete {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    padding: 0.5rem 0.7rem;
    background: none;
    border: none;
    color: var(--text-primary);
    cursor: pointer;
    font-size: 0.87rem;
    text-align: left;
  }
  .tete:hover { background: var(--bg-tertiary); }
  .nom { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sorte {
    color: var(--text-muted);
    font-size: 0.66rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    flex: none;
  }
  .sante { color: var(--text-secondary); font-size: 0.78rem; font-variant-numeric: tabular-nums; flex: none; }
  .sante.incomplet { color: var(--warning); }
  .version {
    padding: 0.04rem 0.45rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 0.7rem;
    flex: none;
  }
  .espace { flex: 1; }

  .pastilles { display: inline-flex; align-items: center; gap: 0.16rem; flex: none; }
  .pastille { width: 7px; height: 7px; border-radius: 2px; flex: none; }
  .pastille.bon { background: var(--success); }
  .pastille.mauvais { background: var(--error); }
  .pastille.attente { background: var(--warning); }
  .pastille.fini { background: var(--text-muted); opacity: 0.55; }
  .reste { color: var(--text-muted); font-size: 0.68rem; margin-left: 0.1rem; }

  .jauge {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    width: 4.6rem;
    height: 1.15rem;
    padding: 0 0.35rem;
    border-radius: var(--radius-sm);
    background: var(--bg-tertiary);
    flex: none;
    overflow: hidden;
  }
  /* **DEUX CHOSES NE PARTAGENT PAS UN NOM DE CLASSE.** Ce remplissage s'est d'abord appele
     `.barre`, comme la barre du haut de l'ecran : celle-ci heritait donc de `position:
     absolute` et partait se coller dans le coin de la fenetre, par-dessus la barre laterale.
     Invisible a la relecture, evident des qu'on encadre les boites. */
  .remplissage { position: absolute; inset: 0 auto 0 0; background: var(--accent-soft); }
  .remplissage.ram { background: var(--success-soft); }
  .valeur {
    position: relative;
    color: var(--text-secondary);
    font-size: 0.72rem;
    font-variant-numeric: tabular-nums;
  }

  .pods { border-top: 1px solid var(--border-color); }
  .pod { display: flex; align-items: center; }
  .pod + .pod { border-top: 1px solid var(--border-color); }
  .pod:hover { background: var(--bg-tertiary); }
  .pod.choisi { background: var(--accent-soft); }

  .ligne {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex: 1;
    min-width: 0;
    padding: 0.32rem 0.7rem 0.32rem 1.6rem;
    background: none;
    border: none;
    color: var(--text-primary);
    cursor: pointer;
    text-align: left;
  }
  .pod-nom {
    font-family: var(--font-mono);
    font-size: 0.76rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pod-etat { color: var(--text-secondary); font-size: 0.76rem; flex: none; }
  .pod-etat.mauvais { color: var(--error); }
  .pod-age, .mesure { color: var(--text-muted); font-size: 0.76rem; font-variant-numeric: tabular-nums; flex: none; }
  .mesure { min-width: 3.4rem; text-align: right; }
  .redemarrages { color: var(--warning); font-size: 0.76rem; flex: none; }
  .actions { display: flex; gap: 0.2rem; padding-right: 0.55rem; }

  /* ── Le detail ────────────────────────────────────────────────────────────── */
  .detail {
    width: min(50%, 44rem);
    display: flex;
    flex-direction: column;
    min-height: 0;
    gap: 0.5rem;
    padding: 0.6rem;
    border: 1px solid var(--border-color);
    border-radius: var(--radius);
    background: var(--surface-base);
  }
  .detail-tete { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; }
  .detail-titre { display: flex; align-items: center; gap: 0.5rem; min-width: 0; }
  .detail-nom {
    font-family: var(--font-mono);
    font-size: 0.82rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .detail-faits {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
    color: var(--text-muted);
    font-size: 0.76rem;
  }
  .onglets { display: flex; gap: 0.15rem; border-bottom: 1px solid var(--border-color); }
  .onglet {
    padding: 0.3rem 0.7rem;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 0.82rem;
  }
  .onglet:hover { color: var(--text-primary); }
  .onglet.actif { color: var(--text-primary); border-bottom-color: var(--accent); }

  .contenu {
    flex: 1;
    min-height: 0;
    overflow: auto;
    margin: 0;
    padding: 0.6rem;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: 0.75rem;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .vide { color: var(--text-muted); text-align: center; padding: 2.5rem 1rem; }
  .vide .aide { font-size: 0.82rem; margin-top: 0.45rem; }
  .vide .erreur { color: var(--error); }
</style>
